# shellcheck shell=bash
# GPU discovery shared by bootstrap.sh and doctor.sh. Sourced, not executed.
#
# nvidia-smi only sees NVIDIA cards. AMD cards are found through sysfs, the
# same source the app's System Control view uses (src-tauri/src/system/gpu.rs).
# Ollama runs on them through its Vulkan backend (Mesa's RADV driver), which
# also covers older cards such as Polaris (RX 470–590) that ROCm dropped.

# Prints one line per AMD GPU: slot|name|kernel driver|VRAM total MiB|VRAM used MiB
# VRAM is 0 when the driver doesn't expose it (only amdgpu does).
amd_gpus() {
  local dev card slot name drv total used
  for dev in /sys/class/drm/card*/device; do
    card="$(basename "$(dirname "$dev")")"
    [[ $card =~ ^card[0-9]+$ ]] || continue          # skip connectors (card1-DP-1, ...)
    [[ "$(cat "$dev/vendor" 2>/dev/null)" == 0x1002 ]] || continue
    slot="$(basename "$(readlink -f "$dev")")"
    name=""
    if command -v lspci >/dev/null 2>&1; then
      # Device field, e.g. 'Ellesmere [Radeon RX 470/480/570/570X/580/580X/590]' → the bracketed marketing name.
      name="$(lspci -mm -s "$slot" 2>/dev/null | awk -F'"' '{print $6}' | sed -E 's/.*\[(.*)\].*/\1/' || true)"
    fi
    drv="$(basename "$(readlink -f "$dev/driver" 2>/dev/null || true)")"
    total=$(( $(cat "$dev/mem_info_vram_total" 2>/dev/null || echo 0) / 1048576 ))
    used=$(( $(cat "$dev/mem_info_vram_used" 2>/dev/null || echo 0) / 1048576 ))
    echo "${slot}|${name:-AMD GPU}|${drv:-none}|${total}|${used}"
  done
}

# True when Mesa's RADV Vulkan driver is installed (Ollama needs it for AMD).
have_radv() {
  compgen -G "/usr/share/vulkan/icd.d/radeon_icd*.json" >/dev/null
}
