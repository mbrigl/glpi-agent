// SPDX-License-Identifier: GPL-2.0-only

//! Parsing of a `RetrieveProperties` response into [`HostInfo`] values.
//!
//! The response is a flat list of `ObjectContent` entries — some `HostSystem`,
//! some `VirtualMachine` — each carrying the property name/value pairs we
//! requested. [`parse_hosts`] turns that into one [`HostInfo`] per host, folding
//! each host's virtual machines in by matching the host's `vm` managed-object
//! references against the `VirtualMachine` objects.

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};
use quick_xml::events::Event;

use crate::host::{HostInfo, VmInfo};
use crate::soap::fault_message;

/// One `ObjectContent` entry from the response: its managed-object reference and
/// the requested properties, split into scalars, managed-object-reference lists
/// and the MAC/IP leaves harvested from nested values.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RawObject {
    obj_type: String,
    obj_id: String,
    scalars: BTreeMap<String, String>,
    refs: BTreeMap<String, Vec<String>>,
    macs: Vec<String>,
    ips: Vec<String>,
}

/// Parses a `RetrieveProperties` response into one [`HostInfo`] per host.
///
/// # Errors
///
/// Returns [`AgentError::Protocol`] if the body is a SOAP fault.
pub fn parse_hosts(xml: &str) -> Result<Vec<HostInfo>> {
    if let Some(message) = fault_message(xml) {
        return Err(AgentError::Protocol(format!("vSphere fault: {message}")));
    }
    let objects = parse_objects(xml);

    // Index the virtual machines by their moref so each host can pick up its own.
    let mut vms: BTreeMap<String, VmInfo> = BTreeMap::new();
    let mut hosts: Vec<RawObject> = Vec::new();
    for object in objects {
        match object.obj_type.as_str() {
            "VirtualMachine" => {
                vms.insert(object.obj_id.clone(), vm_from_object(&object));
            }
            "HostSystem" => hosts.push(object),
            _ => {}
        }
    }

    Ok(hosts
        .iter()
        .map(|host| host_from_object(host, &vms))
        .collect())
}

/// Builds a [`HostInfo`] from a `HostSystem` object, attaching the VMs whose
/// morefs appear in its `vm` reference list.
fn host_from_object(host: &RawObject, vms: &BTreeMap<String, VmInfo>) -> HostInfo {
    let get = |name: &str| host.scalars.get(name).cloned();
    let num = |name: &str| scalar(&host.scalars, name);
    let count = |name: &str| scalar::<u32>(&host.scalars, name);

    let virtual_machines = host
        .refs
        .get("vm")
        .into_iter()
        .flatten()
        .filter_map(|moref| vms.get(moref).cloned())
        .collect();

    HostInfo {
        // Prefer the DNS host name, then the summary name, then the moref name.
        name: get("config.network.dnsConfig.hostName")
            .or_else(|| get("summary.config.name"))
            .or_else(|| get("name")),
        product_full_name: get("config.product.fullName"),
        product_version: get("config.product.version"),
        uuid: get("hardware.systemInfo.uuid"),
        vendor: get("hardware.systemInfo.vendor"),
        model: get("hardware.systemInfo.model"),
        serial: get("hardware.systemInfo.serialNumber"),
        bios_version: get("hardware.biosInfo.biosVersion"),
        bios_date: get("hardware.biosInfo.releaseDate"),
        cpu_model: get("summary.hardware.cpuModel"),
        cpu_mhz: num("summary.hardware.cpuMhz"),
        cpu_packages: count("summary.hardware.numCpuPkgs"),
        cpu_cores: count("summary.hardware.numCpuCores"),
        cpu_threads: count("summary.hardware.numCpuThreads"),
        memory_bytes: num("hardware.memorySize"),
        interfaces: Vec::new(),
        virtual_machines,
    }
}

/// Builds a [`VmInfo`] from a `VirtualMachine` object.
fn vm_from_object(vm: &RawObject) -> VmInfo {
    let get = |name: &str| vm.scalars.get(name).cloned();
    VmInfo {
        name: get("summary.config.name"),
        uuid: get("summary.config.uuid"),
        vcpu: scalar::<u32>(&vm.scalars, "summary.config.numCpu"),
        memory_mb: scalar::<u64>(&vm.scalars, "summary.config.memorySizeMB"),
        power_state: get("summary.runtime.powerState"),
        guest_full_name: get("summary.config.guestFullName")
            .or_else(|| get("summary.guest.guestFullName")),
        comment: get("summary.config.annotation"),
        mac_addresses: vm.macs.clone(),
        ip_addresses: {
            // The guest's reported address plus any harvested from nested values.
            let mut ips = Vec::new();
            if let Some(ip) = get("summary.guest.ipAddress") {
                if !ip.is_empty() {
                    ips.push(ip);
                }
            }
            for ip in &vm.ips {
                if !ips.contains(ip) {
                    ips.push(ip.clone());
                }
            }
            ips
        },
    }
}

/// Parses the scalar property `name` from `scalars` into any `FromStr` type.
fn scalar<T: std::str::FromStr>(scalars: &BTreeMap<String, String>, name: &str) -> Option<T> {
    scalars.get(name).and_then(|v| v.parse().ok())
}

/// Walks the response, accumulating one [`RawObject`] per `<obj>` boundary.
fn parse_objects(xml: &str) -> Vec<RawObject> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut objects = Vec::new();
    let mut current: Option<RawObject> = None;
    // The element name stack (local names, namespace prefixes stripped).
    let mut stack: Vec<String> = Vec::new();
    // The property name of the `<propSet>` currently being read.
    let mut prop_name: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.local_name().as_ref());
                if local == "obj" {
                    // A new object: commit the previous one, start fresh.
                    if let Some(done) = current.take() {
                        objects.push(done);
                    }
                    let obj_type = attr(&e, "type").unwrap_or_default();
                    current = Some(RawObject {
                        obj_type,
                        ..RawObject::default()
                    });
                    prop_name = None;
                }
                stack.push(local);
            }
            Ok(Event::Empty(e)) => {
                // Self-closing managed-object refs can appear (rare); record id.
                let local = local_name(e.local_name().as_ref());
                if local == "ManagedObjectReference" {
                    if let (Some(obj), Some(name)) = (current.as_mut(), prop_name.as_ref()) {
                        if let Some(id) = attr(&e, "value") {
                            obj.refs.entry(name.clone()).or_default().push(id);
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().unwrap_or_default().trim().to_owned();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                route_text(&stack, &mut current, &mut prop_name, text);
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if let Some(done) = current {
        objects.push(done);
    }
    objects
}

/// Routes a text node to the right slot of the current object based on the
/// element stack.
fn route_text(
    stack: &[String],
    current: &mut Option<RawObject>,
    prop_name: &mut Option<String>,
    text: String,
) {
    let Some(top) = stack.last().map(String::as_str) else {
        return;
    };
    let Some(obj) = current.as_mut() else {
        return;
    };
    match top {
        "obj" => obj.obj_id.push_str(&text),
        // `<name>` is the property name only directly under `<propSet>`.
        "name" if stack.iter().rev().nth(1).map(String::as_str) == Some("propSet") => {
            *prop_name = Some(text);
        }
        "ManagedObjectReference" => {
            if let Some(name) = prop_name.as_ref() {
                obj.refs.entry(name.clone()).or_default().push(text);
            }
        }
        "macAddress" => obj.macs.push(text),
        "ipAddress" => obj.ips.push(text),
        // A scalar `<val>` directly carries the property's value.
        "val" => {
            if let Some(name) = prop_name.as_ref() {
                obj.scalars
                    .entry(name.clone())
                    .and_modify(|v| {
                        v.push(' ');
                        v.push_str(&text);
                    })
                    .or_insert(text);
            }
        }
        _ => {}
    }
}

/// Strips an optional namespace prefix from a raw element name.
fn local_name(raw: &[u8]) -> String {
    let name = String::from_utf8_lossy(raw);
    name.rsplit(':').next().unwrap_or(&name).to_owned()
}

/// Returns the value of attribute `name` on a start tag, if present.
fn attr(e: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (local_name(a.key.as_ref()) == name).then(|| String::from_utf8_lossy(&a.value).into_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::parse_hosts;

    const RESPONSE: &str = "\
<RetrievePropertiesResponse xmlns=\"urn:vim25\">\
<returnval>\
<obj type=\"VirtualMachine\">vm-1</obj>\
<propSet><name>summary.config.name</name><val xsi:type=\"xsd:string\">db01</val></propSet>\
<propSet><name>summary.config.uuid</name><val xsi:type=\"xsd:string\">420d-vm</val></propSet>\
<propSet><name>summary.config.numCpu</name><val xsi:type=\"xsd:int\">4</val></propSet>\
<propSet><name>summary.config.memorySizeMB</name><val xsi:type=\"xsd:int\">8192</val></propSet>\
<propSet><name>summary.runtime.powerState</name><val xsi:type=\"VirtualMachinePowerState\">poweredOn</val></propSet>\
<propSet><name>summary.guest.guestFullName</name><val xsi:type=\"xsd:string\">Ubuntu Linux (64-bit)</val></propSet>\
<propSet><name>summary.guest.ipAddress</name><val xsi:type=\"xsd:string\">10.0.0.20</val></propSet>\
</returnval>\
<returnval>\
<obj type=\"HostSystem\">host-9</obj>\
<propSet><name>config.network.dnsConfig.hostName</name><val xsi:type=\"xsd:string\">esx1</val></propSet>\
<propSet><name>config.product.fullName</name><val xsi:type=\"xsd:string\">VMware ESXi 7.0.3 build-1</val></propSet>\
<propSet><name>hardware.systemInfo.vendor</name><val xsi:type=\"xsd:string\">Dell Inc.</val></propSet>\
<propSet><name>hardware.memorySize</name><val xsi:type=\"xsd:long\">68719476736</val></propSet>\
<propSet><name>summary.hardware.numCpuPkgs</name><val xsi:type=\"xsd:short\">2</val></propSet>\
<propSet><name>summary.hardware.numCpuCores</name><val xsi:type=\"xsd:short\">40</val></propSet>\
<propSet><name>vm</name><val xsi:type=\"ArrayOfManagedObjectReference\">\
<ManagedObjectReference type=\"VirtualMachine\">vm-1</ManagedObjectReference>\
</val></propSet>\
</returnval>\
</RetrievePropertiesResponse>";

    #[test]
    fn parses_host_with_its_vm() {
        let hosts = parse_hosts(RESPONSE).unwrap();
        assert_eq!(hosts.len(), 1);
        let host = &hosts[0];
        assert_eq!(host.name.as_deref(), Some("esx1"));
        assert_eq!(host.vendor.as_deref(), Some("Dell Inc."));
        assert_eq!(host.memory_bytes, Some(68_719_476_736));
        assert_eq!(host.cpu_packages, Some(2));
        assert_eq!(host.cpu_cores, Some(40));

        assert_eq!(host.virtual_machines.len(), 1);
        let vm = &host.virtual_machines[0];
        assert_eq!(vm.name.as_deref(), Some("db01"));
        assert_eq!(vm.vcpu, Some(4));
        assert_eq!(vm.memory_mb, Some(8192));
        assert_eq!(vm.power_state.as_deref(), Some("poweredOn"));
        assert_eq!(vm.guest_full_name.as_deref(), Some("Ubuntu Linux (64-bit)"));
        assert_eq!(vm.ip_addresses, vec!["10.0.0.20".to_owned()]);
    }

    #[test]
    fn fault_response_is_an_error() {
        let fault = "<soapenv:Fault><faultstring>boom</faultstring></soapenv:Fault>";
        assert!(parse_hosts(fault).is_err());
    }

    #[test]
    fn unrelated_vm_is_not_attached() {
        // A VM whose moref the host does not list must not be folded in.
        let xml = RESPONSE.replace(
            "vm-1</ManagedObjectReference>",
            "vm-99</ManagedObjectReference>",
        );
        let hosts = parse_hosts(&xml).unwrap();
        assert!(hosts[0].virtual_machines.is_empty());
    }
}
