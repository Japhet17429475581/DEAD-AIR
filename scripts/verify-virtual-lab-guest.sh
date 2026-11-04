#!/bin/sh
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
mount -t tmpfs -o mode=0755 tmpfs /run
mount -t tmpfs -o mode=1777 tmpfs /tmp
mkdir -p /dev/shm
mount -t tmpfs -o mode=1777 tmpfs /dev/shm
helper=
for item in $(cat /proc/cmdline); do case "$item" in deadair_helper=*) helper=${item#deadair_helper=};; esac; done
echo "DEADAIR_VM_KERNEL=$(uname -r)"
modprobe mac80211_hwsim radios=2
if [ -f /work/scripts/verify-virtual-lab.py ]; then
  python3 /work/scripts/verify-virtual-lab.py "$helper"
  result=$?
else
  iw dev
  result=0
fi
if [ "$result" -eq 0 ]; then echo DEADAIR_VM_RESULT=PASS; else echo DEADAIR_VM_RESULT=FAIL; fi
sync
/bin/busybox poweroff -f
