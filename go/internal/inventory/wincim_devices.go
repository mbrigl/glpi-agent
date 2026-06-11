// SPDX-License-Identifier: GPL-2.0-only

package inventory

var (
	winVideoProperties = []string{
		"CurrentHorizontalResolution", "CurrentVerticalResolution",
		"VideoProcessor", "AdapterRAM", "Name", "PNPDeviceID",
	}
	winSoundProperties = []string{"Name", "Manufacturer", "Caption", "Description"}
)

// buildWinVideos maps Win32_VideoController to VIDEOS, mirroring Win32/Videos.pm:
// CHIPSET, NAME (required, deduplicated), MEMORY (AdapterRAM bytes -> MiB) and
// RESOLUTION "<H>x<V>". The registry MemorySize override is follow-on.
func buildWinVideos(objects []map[string]any) []map[string]any {
	var videos []map[string]any
	seen := map[string]bool{}
	for _, o := range objects {
		name := cimString(o, "Name")
		if name == "" || seen[name] {
			continue
		}
		seen[name] = true
		v := map[string]any{"NAME": name}
		setIf(v, "CHIPSET", cimString(o, "VideoProcessor"))
		if mem := cimBytesToMB(o, "AdapterRAM"); mem > 0 {
			v["MEMORY"] = mem
		}
		if h := cimInt(o, "CurrentHorizontalResolution"); h > 0 {
			v["RESOLUTION"] = cimString(o, "CurrentHorizontalResolution") + "x" + cimString(o, "CurrentVerticalResolution")
		}
		videos = append(videos, v)
	}
	return videos
}

// buildWinSounds maps Win32_SoundDevice to SOUNDS, mirroring Win32/Sounds.pm.
func buildWinSounds(objects []map[string]any) []map[string]any {
	var sounds []map[string]any
	for _, o := range objects {
		snd := map[string]any{}
		setIf(snd, "NAME", cimString(o, "Name"))
		setIf(snd, "CAPTION", cimString(o, "Caption"))
		setIf(snd, "MANUFACTURER", cimString(o, "Manufacturer"))
		setIf(snd, "DESCRIPTION", cimString(o, "Description"))
		if len(snd) > 0 {
			sounds = append(sounds, snd)
		}
	}
	return sounds
}
