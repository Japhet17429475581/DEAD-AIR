#!/usr/bin/env bash
# Debian-container test harness. The VM sees a read-only root and no NIC.
set -euo pipefail
[[ -f /.dockerenv ]] || { echo 'Run this harness inside a disposable Debian test container.' >&2; exit 1; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
kernel_file=$(find /boot -maxdepth 1 -name 'vmlinuz-*-amd64' -type f | sort -V | tail -n 1)
[[ -n $kernel_file ]] || { echo 'Install a Debian linux-image-amd64 and qemu-system-x86 in the test container.' >&2; exit 1; }
kernel=${kernel_file##*/vmlinuz-}
helper=${1:-${CARGO_TARGET_DIR:-"$root/src-tauri/target"}/debug/deadair-capture-helper}
[[ -x $helper && $helper =~ ^/[A-Za-z0-9_./-]+$ ]] || { echo 'Pass an absolute built helper path.' >&2; exit 1; }
work="$root/artifacts/virtual-lab-vm"
mkdir -p "$work"
ram=$(mktemp -d "$work/initrd.XXXXXX")
mkdir -p "$ram"/{bin,sbin,lib/modules,proc,sys,dev,newroot}
cp /bin/busybox "$ram/bin/busybox"
ln -s busybox "$ram/bin/sh"
for module in virtio_pci 9pnet_virtio 9p; do
  while read -r action path rest; do
    [[ $action != insmod ]] || cp --parents "$path" "$ram/"
  done < <(modprobe --set-version "$kernel" --show-depends "$module")
done
for file in /lib/modules/"$kernel"/modules.builtin* /lib/modules/"$kernel"/modules.order; do
  [[ ! -f $file ]] || cp "$file" "$ram/lib/modules/$kernel/"
done
depmod -b "$ram" "$kernel"
cat > "$ram/init" <<'INIT'
#!/bin/sh
export PATH=/bin:/sbin
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sysfs /sys
/bin/busybox mount -t devtmpfs devtmpfs /dev
/bin/busybox modprobe virtio_pci
/bin/busybox modprobe 9pnet_virtio
/bin/busybox modprobe 9p
/bin/busybox mount -t 9p -o ro,trans=virtio,version=9p2000.L labroot /newroot || exec /bin/sh
/bin/busybox mount --move /proc /newroot/proc
/bin/busybox mount --move /sys /newroot/sys
/bin/busybox mount --move /dev /newroot/dev
exec /bin/busybox switch_root /newroot /work/scripts/verify-virtual-lab-guest.sh
INIT
chmod 0755 "$ram/init"
(cd "$ram"; find . -print0 | cpio --null -o --format=newc --quiet | gzip -1 > "$work/initrd.gz")
set +e
timeout 240 qemu-system-x86_64 -accel tcg -m 1024 -smp 2 -display none -monitor none -serial stdio -no-reboot -nic none \
  -kernel "$kernel_file" -initrd "$work/initrd.gz" \
  -append "console=ttyS0 quiet panic=-1 deadair_helper=$helper" \
  -fsdev local,id=labroot,path=/,security_model=none,readonly=on,multidevs=remap \
  -device virtio-9p-pci,fsdev=labroot,mount_tag=labroot > "$work/serial.log" 2>&1
code=$?
set -e
cat "$work/serial.log"
grep -q 'DEADAIR_VM_RESULT=PASS' "$work/serial.log" || exit 1
[[ $code == 0 ]] || exit "$code"
