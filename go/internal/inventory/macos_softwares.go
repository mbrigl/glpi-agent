// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"
)

var macFormatDateRE = regexp.MustCompile(`^\s*(\d{1,2})/(\d{1,2})/(\d{2})`)

// macFormatDate reformats a "D/M/YY ..." system_profiler date to "MM/DD/YYYY"
// (swapping day/month), mirroring Tools/MacOS.pm _formatDate. A non-matching
// string is returned unchanged.
func macFormatDate(s string) string {
	m := macFormatDateRE.FindStringSubmatch(s)
	if m == nil {
		return s
	}
	d, _ := strconv.Atoi(m[1])
	mo, _ := strconv.Atoi(m[2])
	y, _ := strconv.Atoi(m[3])
	return pad2(mo) + "/" + pad2(d) + "/" + strconv.Itoa(2000+y)
}

func pad2(n int) string {
	if n < 10 {
		return "0" + strconv.Itoa(n)
	}
	return strconv.Itoa(n)
}

var macOffsetDateRE = regexp.MustCompile(`^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$`)

// macOffsetDate converts an ISO UTC "lastModified" plus a local-time offset
// (seconds) to "DD/MM/YYYY", mirroring Tools/MacOS.pm _getOffsetDate.
func macOffsetDate(lastmod string, offset int) string {
	m := macOffsetDateRE.FindStringSubmatch(lastmod)
	if m == nil {
		return ""
	}
	y, _ := strconv.Atoi(m[1])
	mo, _ := strconv.Atoi(m[2])
	d, _ := strconv.Atoi(m[3])
	h, _ := strconv.Atoi(m[4])
	mi, _ := strconv.Atoi(m[5])
	se, _ := strconv.Atoi(m[6])
	t := time.Date(y, time.Month(mo), d, h, mi, se, 0, time.UTC).Add(time.Duration(offset) * time.Second).UTC()
	return t.Format("02/01/2006")
}

// macArchKind maps a plist arch_kind to the GLPI Kind value.
var macArchKind = map[string]string{
	"arch_arm_i64": "Universal",
	"arch_arm":     "Arm",
	"arch_i64":     "Intel (x86_64)",
	"arch_i32":     "Intel (i386)",
	"arch_other":   "Other",
}

// extractMacSoftwaresFromXML normalises the SPApplicationsDataType plist into the
// uniform name->attributes map the text parser also produces, mirroring
// Tools/MacOS.pm _extractSoftwaresFromXml.
func extractMacSoftwaresFromXML(root any, offset int) map[string]map[string]any {
	softlist := plistDictArray(root, "")
	softwares := map[string]map[string]any{}

	for _, item := range softlist {
		soft, ok := item.(map[string]any)
		if !ok {
			continue
		}
		name := plistStr(soft, "_name")
		if name == "" {
			continue
		}

		entry := map[string]any{}

		if env, ok := soft["runtime_environment"].(string); ok {
			if env == "arch_x86" {
				entry["Kind"] = "Intel"
			} else {
				entry["Kind"] = ucfirstKeep(env)
			}
		}
		if kind, ok := macArchKind[plistStr(soft, "arch_kind")]; ok {
			entry["Kind"] = kind
		}
		if lastmod, ok := soft["lastModified"].(string); ok {
			if d := macOffsetDate(lastmod, offset); d != "" {
				entry["Last Modified"] = d
			}
		}
		if signers, ok := soft["signed_by"].([]any); ok {
			for _, s := range signers {
				if str, ok := s.(string); ok && strings.HasPrefix(str, "Developer ID Application:") {
					entry["Signed by"] = str
					break
				}
			}
		}
		if plistStr(soft, "obtained_from") == "identified_developer" {
			entry["Obtained from"] = "Identified Developer"
		}
		for src, dst := range map[string]string{"version": "Version", "path": "Location", "info": "Get Info String"} {
			if v, ok := soft[src].(string); ok {
				entry[dst] = v
			}
		}

		key := name
		for i := 0; ; i++ {
			if _, exists := softwares[key]; !exists {
				break
			}
			key = name + "_" + strconv.Itoa(i)
		}
		softwares[key] = entry
	}
	return softwares
}

func ucfirstKeep(s string) string {
	if s == "" {
		return ""
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

var (
	macParallelsWinRE = regexp.MustCompile(`^\S+, [A-Z]:\\`)
	macDeveloperRE    = regexp.MustCompile(`^Developer ID Application: ([^,]*),?`)
	macParenSuffixRE  = regexp.MustCompile(`^(.*)\s+\(.*\)$`)
	macIncRE          = regexp.MustCompile(`(?i)\s*Incorporated.*`)
	macCorpRE         = regexp.MustCompile(`(?i)\s*Corporation.*`)
	macAppleWordRE    = regexp.MustCompile(`(?i)\bApple\b`)
	macCopyrightRE    = regexp.MustCompile(`(?i)(\(C\)|\x{00a9}|Copyright|\x{fffd})`)
	macByRE           = regexp.MustCompile(`(?i)\sby\s(.*)`)
	macUpToCopyRE     = regexp.MustCompile(`(?i).*(\(C\)|\x{00a9}|Copyright|\x{fffd})\s*`)
	macAllRightsRE    = regexp.MustCompile(`(?i)\s*All rights reserved\.?\s*`)
	macYearsRE        = regexp.MustCompile(`\s*\d+(\s*-\s*\d+)?\s*`)
	macSysLibRE       = regexp.MustCompile(`/System/Library/(CoreServices|Frameworks)/`)
	macVerSpaceDotRE  = regexp.MustCompile(` \. `)
)

// buildMacSoftwares maps the uniform application attribute map to the SOFTWARES
// section, mirroring MacOS/Softwares.pm _getSoftwaresList: NAME/VERSION, the
// PUBLISHER heuristics (Obtained from / Signed by / Get Info String copyright /
// canonical-manufacturer fallback), INSTALLDATE, ARCH and the SYSTEM_CATEGORY/
// USERNAME extracted from the install Location. Entries are sorted by NAME.
func buildMacSoftwares(apps map[string]map[string]any) []map[string]any {
	names := make([]string, 0, len(apps))
	for n := range apps {
		names = append(names, n)
	}
	sort.Strings(names)

	var softwares []map[string]any
	for _, name := range names {
		app := apps[name]
		getInfo := appStr(app, "Get Info String")

		// Windows application found by Parallels (issue #716).
		if getInfo != "" && macParallelsWinRE.MatchString(getInfo) {
			continue
		}

		soft := map[string]any{"NAME": name}
		if version := macVerSpaceDotRE.ReplaceAllString(appStr(app, "Version"), "."); version != "" {
			soft["VERSION"] = version
		}

		if pub := macSoftwarePublisher(name, app, getInfo); pub != "" {
			soft["PUBLISHER"] = pub
		}
		setIf(soft, "INSTALLDATE", appStr(app, "Last Modified"))
		setIf(soft, "ARCH", appStr(app, "Kind"))

		category, username := macSoftwareCategoryUser(appStr(app, "Location"))
		setIf(soft, "SYSTEM_CATEGORY", category)
		setIf(soft, "USERNAME", username)

		softwares = append(softwares, soft)
	}
	return softwares
}

// macSoftwarePublisher derives the PUBLISHER field per the upstream heuristics.
func macSoftwarePublisher(name string, app map[string]any, getInfo string) string {
	source := appStr(app, "Obtained from")
	location := appStr(app, "Location")

	if source == "Apple" || (location != "" && macSysLibRE.MatchString(location)) {
		return "Apple"
	}
	if source == "Identified Developer" {
		if signed := appStr(app, "Signed by"); signed != "" {
			if m := macDeveloperRE.FindStringSubmatch(signed); m != nil {
				dev := m[1]
				if sm := macParenSuffixRE.FindStringSubmatch(dev); sm != nil {
					dev = sm[1]
				}
				dev = macIncRE.ReplaceAllString(dev, " Inc.")
				dev = macCorpRE.ReplaceAllString(dev, "")
				if dev != "" {
					return dev
				}
			}
		}
	}

	if getInfo != "" {
		parts := regexp.MustCompile(`,\s+`).Split(getInfo, -1)
		for _, p := range parts {
			if macAppleWordRE.MatchString(p) {
				return "Apple"
			}
		}
		var publisher string
		for _, p := range parts {
			if macCopyrightRE.MatchString(p) {
				publisher = p
				break
			}
		}
		if publisher != "" {
			if m := macByRE.FindStringSubmatch(publisher); m != nil {
				publisher = m[1]
			}
			publisher = macUpToCopyRE.ReplaceAllString(publisher, "")
			publisher = macAllRightsRE.ReplaceAllString(publisher, "")
			publisher = macIncRE.ReplaceAllString(publisher, " Inc.")
			publisher = macCorpRE.ReplaceAllString(publisher, "")
			publisher = macYearsRE.ReplaceAllString(publisher, "")
			if publisher != "" {
				return publisher
			}
		}
		if editor := getCanonicalManufacturer(getInfo); editor != getInfo {
			return editor
		}
	}

	if editor := getCanonicalManufacturer(name); editor != name {
		return editor
	}
	return ""
}

var (
	macUserCat2RE = regexp.MustCompile(`^/Users/([^/]+)/([^/]+/[^/]+)/`)
	macUserCat1RE = regexp.MustCompile(`^/Users/([^/]+)/([^/]+)/`)
	macVolCat2RE  = regexp.MustCompile(`^/Volumes/[^/]+/([^/]+/[^/]+)/`)
	macVolCat1RE  = regexp.MustCompile(`^/Volumes/[^/]+/([^/]+)/`)
	macRootCat2RE = regexp.MustCompile(`^/([^/]+/[^/]+)/`)
	macRootCat1RE = regexp.MustCompile(`^/([^/]+)/`)
	macDownDeskRE = regexp.MustCompile(`^Downloads|^Desktop`)
)

// macSoftwareCategoryUser extracts SYSTEM_CATEGORY and USERNAME from the install
// Location, mirroring MacOS/Softwares.pm _extractSoftwareSystemCategoryAndUserName.
func macSoftwareCategoryUser(str string) (category, username string) {
	if str == "" {
		return "", ""
	}
	if m := macUserCat2RE.FindStringSubmatch(str); m != nil {
		username = m[1]
		if !macDownDeskRE.MatchString(m[2]) {
			category = m[2]
		}
		return category, username
	}
	if m := macUserCat1RE.FindStringSubmatch(str); m != nil {
		username = m[1]
		if !macDownDeskRE.MatchString(m[2]) {
			category = m[2]
		}
		return category, username
	}
	for _, re := range []*regexp.Regexp{macVolCat2RE, macVolCat1RE, macRootCat2RE, macRootCat1RE} {
		if m := re.FindStringSubmatch(str); m != nil {
			return m[1], ""
		}
	}
	return "", ""
}

// appStr returns a string attribute from an application entry.
func appStr(app map[string]any, key string) string {
	if app == nil {
		return ""
	}
	s, _ := app[key].(string)
	return s
}

// extractMacSoftwaresFromText pulls the application attribute maps from a parsed
// text SPApplicationsDataType tree (the "Applications" node), the text-format
// counterpart of extractMacSoftwaresFromXML.
func extractMacSoftwaresFromText(info map[string]any) map[string]map[string]any {
	appsNode, _ := info["Applications"].(map[string]any)
	out := map[string]map[string]any{}
	for name, v := range appsNode {
		if m, ok := v.(map[string]any); ok {
			out[name] = m
		}
	}
	return out
}
