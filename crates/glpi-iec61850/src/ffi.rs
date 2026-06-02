// SPDX-License-Identifier: GPL-2.0-only

//! libiec61850 FFI backend for [`IedProtocol`].
//!
//! This module is compiled only with the off-by-default `libiec61850` feature,
//! which links the system **libiec61850** C client library (v1.6.x). It
//! provides [`LibIec61850Protocol`], a real on-wire MMS transport implementing
//! the same [`IedProtocol`] seam the scan logic in [`crate::device`] runs over.
//!
//! The bindings are hand-written `extern "C"` declarations against the
//! libiec61850 1.6 client API (`iec61850_client.h`), so no `bindgen`/header
//! step is needed at build time — only the shared library at link time (the
//! [`build.rs`](../build.rs) link directive is gated on the same feature).
//!
//! # Safety
//!
//! Every call into the C library is `unsafe`; the wrapper owns the
//! `IedConnection` handle and frees it on drop, and copies every returned C
//! string into an owned [`String`] before the library buffer can be reused.
//! The handle is used from a single task, so [`LibIec61850Protocol`] asserts
//! [`Send`] for the opaque connection pointer.

#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};

use crate::protocol::{FunctionalConstraint, IedProtocol};

// --- libiec61850 1.6 client API (subset) -----------------------------------

/// Opaque `IedConnection` handle.
type IedConnection = *mut c_void;
/// Opaque `LinkedList` handle (directory listings).
type LinkedList = *mut c_void;
/// Opaque `MmsValue` handle (a read value).
type MmsValue = *mut c_void;
/// `IedClientError`; `IED_ERROR_OK` is `0`.
type IedClientError = c_int;

/// `IED_ERROR_OK`.
const IED_ERROR_OK: IedClientError = 0;
/// `ACSI_CLASS_DATA_OBJECT` — the first `ACSIClass` enumerator.
const ACSI_CLASS_DATA_OBJECT: c_int = 0;
/// `IEC61850_FC_DC` — description constants (the `PhyNam` functional constraint).
const IEC61850_FC_DC: c_int = 5;

#[allow(non_snake_case)]
extern "C" {
    fn IedConnection_create() -> IedConnection;
    fn IedConnection_connect(
        self_: IedConnection,
        error: *mut IedClientError,
        hostname: *const c_char,
        tcpPort: c_int,
    );
    fn IedConnection_close(self_: IedConnection);
    fn IedConnection_destroy(self_: IedConnection);

    fn IedConnection_getLogicalDeviceList(
        self_: IedConnection,
        error: *mut IedClientError,
    ) -> LinkedList;
    fn IedConnection_getLogicalDeviceDirectory(
        self_: IedConnection,
        error: *mut IedClientError,
        logicalDeviceName: *const c_char,
    ) -> LinkedList;
    fn IedConnection_getLogicalNodeDirectory(
        self_: IedConnection,
        error: *mut IedClientError,
        logicalNodeReference: *const c_char,
        acsiClass: c_int,
    ) -> LinkedList;
    fn IedConnection_readObject(
        self_: IedConnection,
        error: *mut IedClientError,
        objectReference: *const c_char,
        fc: c_int,
    ) -> MmsValue;

    fn LinkedList_getNext(self_: LinkedList) -> LinkedList;
    fn LinkedList_getData(self_: LinkedList) -> *mut c_void;
    fn LinkedList_destroy(self_: LinkedList);

    fn MmsValue_printToBuffer(
        self_: MmsValue,
        buffer: *mut c_char,
        bufferSize: c_int,
    ) -> *const c_char;
    fn MmsValue_delete(self_: MmsValue);
}

/// A live IEC 61850 connection backed by libiec61850.
pub struct LibIec61850Protocol {
    connection: SendConnection,
}

/// Wrapper asserting the connection handle is used from a single task.
struct SendConnection(IedConnection);
// SAFETY: the handle is owned by one task; we never share it across threads.
unsafe impl Send for SendConnection {}

impl LibIec61850Protocol {
    /// Connects to the IED at `host:port`.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Transport`] if the connection cannot be created or
    /// the MMS association fails.
    pub fn connect(host: &str, port: u16) -> Result<Self> {
        let hostname = CString::new(host)
            .map_err(|_| AgentError::Transport("host contains a NUL byte".to_owned()))?;
        // SAFETY: create returns a valid handle or null; we check below.
        let connection = unsafe { IedConnection_create() };
        if connection.is_null() {
            return Err(AgentError::Transport(
                "IedConnection_create returned null".to_owned(),
            ));
        }
        let mut error: IedClientError = IED_ERROR_OK;
        // SAFETY: `connection` is valid; `hostname` outlives the call.
        unsafe {
            IedConnection_connect(connection, &mut error, hostname.as_ptr(), c_int::from(port));
        }
        if error != IED_ERROR_OK {
            // SAFETY: destroy a created-but-unconnected handle.
            unsafe { IedConnection_destroy(connection) };
            return Err(AgentError::Transport(format!(
                "IEC 61850 connect to {host}:{port} failed (error {error})"
            )));
        }
        Ok(Self {
            connection: SendConnection(connection),
        })
    }

    /// The raw connection pointer.
    fn handle(&self) -> IedConnection {
        self.connection.0
    }
}

impl Drop for LibIec61850Protocol {
    fn drop(&mut self) {
        // SAFETY: the handle was created in `connect` and not yet destroyed.
        unsafe {
            IedConnection_close(self.handle());
            IedConnection_destroy(self.handle());
        }
    }
}

#[async_trait]
impl IedProtocol for LibIec61850Protocol {
    async fn server_directory(&mut self) -> Result<Vec<String>> {
        let mut error: IedClientError = IED_ERROR_OK;
        // SAFETY: valid handle; `error` is a live out-pointer.
        let list = unsafe { IedConnection_getLogicalDeviceList(self.handle(), &mut error) };
        collect_string_list(list, error, "getLogicalDeviceList")
    }

    async fn logical_device_directory(&mut self, device: &str) -> Result<Vec<String>> {
        let name = c_string(device)?;
        let mut error: IedClientError = IED_ERROR_OK;
        // SAFETY: valid handle; `name` outlives the call.
        let list = unsafe {
            IedConnection_getLogicalDeviceDirectory(self.handle(), &mut error, name.as_ptr())
        };
        collect_string_list(list, error, "getLogicalDeviceDirectory")
    }

    async fn logical_node_directory(&mut self, logical_node: &str) -> Result<Vec<String>> {
        let reference = c_string(logical_node)?;
        let mut error: IedClientError = IED_ERROR_OK;
        // SAFETY: valid handle; `reference` outlives the call.
        let list = unsafe {
            IedConnection_getLogicalNodeDirectory(
                self.handle(),
                &mut error,
                reference.as_ptr(),
                ACSI_CLASS_DATA_OBJECT,
            )
        };
        collect_string_list(list, error, "getLogicalNodeDirectory")
    }

    async fn read_string(
        &mut self,
        reference: &str,
        fc: FunctionalConstraint,
    ) -> Result<Option<String>> {
        let object = c_string(reference)?;
        let mut error: IedClientError = IED_ERROR_OK;
        let fc_code = match fc {
            FunctionalConstraint::DC => IEC61850_FC_DC,
        };
        // SAFETY: valid handle; `object` outlives the call.
        let value = unsafe {
            IedConnection_readObject(self.handle(), &mut error, object.as_ptr(), fc_code)
        };
        if error != IED_ERROR_OK || value.is_null() {
            // A per-reference read failure is "value absent", not fatal.
            if !value.is_null() {
                // SAFETY: non-null value owned by us.
                unsafe { MmsValue_delete(value) };
            }
            return Ok(None);
        }
        let mut buffer = [0_i8; 256];
        // SAFETY: value is non-null; buffer is writable for its full length.
        unsafe {
            MmsValue_printToBuffer(
                value,
                buffer.as_mut_ptr().cast::<c_char>(),
                buffer.len() as c_int,
            );
        }
        // SAFETY: printToBuffer NUL-terminates within the buffer.
        let text = unsafe { CStr::from_ptr(buffer.as_ptr().cast::<c_char>()) }
            .to_string_lossy()
            .trim()
            .to_owned();
        // SAFETY: value is non-null and owned by us.
        unsafe { MmsValue_delete(value) };
        Ok((!text.is_empty()).then_some(text))
    }
}

/// Builds a `CString`, mapping an embedded NUL to a transport error.
fn c_string(value: &str) -> Result<CString> {
    CString::new(value).map_err(|_| AgentError::Transport(format!("{value:?} contains a NUL byte")))
}

/// Drains a libiec61850 `LinkedList` of C strings into owned `String`s, then
/// frees it. The list head is a sentinel, so iteration starts at its successor.
fn collect_string_list(list: LinkedList, error: IedClientError, op: &str) -> Result<Vec<String>> {
    if error != IED_ERROR_OK {
        if !list.is_null() {
            // SAFETY: non-null list returned by the library.
            unsafe { LinkedList_destroy(list) };
        }
        return Err(AgentError::Transport(format!(
            "IEC 61850 {op} failed (error {error})"
        )));
    }
    if list.is_null() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    // SAFETY: `list` is a valid head; we walk via getNext until null.
    let mut node = unsafe { LinkedList_getNext(list) };
    while !node.is_null() {
        // SAFETY: each node's data is a NUL-terminated C string (an entry name).
        let data = unsafe { LinkedList_getData(node) }.cast::<c_char>();
        if !data.is_null() {
            // SAFETY: `data` points at a library-owned NUL-terminated string.
            let name = unsafe { CStr::from_ptr(data) }
                .to_string_lossy()
                .into_owned();
            if !name.is_empty() {
                out.push(name);
            }
        }
        // SAFETY: walking the list.
        node = unsafe { LinkedList_getNext(node) };
    }
    // SAFETY: free the whole list (and its copied-out data) once.
    unsafe { LinkedList_destroy(list) };
    Ok(out)
}
