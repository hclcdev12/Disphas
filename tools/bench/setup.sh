#!/bin/sh
# disphas test bench ONLY: the amnesic live system booted over PXE
# (oblivio-bench, 192.168.1.234). NEVER run this on a real workstation: it
# gives the bench account root without a password (CLAUDE.md §2).
#
# Run by the owner after each boot, or built into the PXE image:
#   sudo sh setup.sh
# It lets the bench account (and Claude, over SSH) run root commands and
# start the disphas helper without a password, and drive the interface.
set -eu
U=goupil

# Root without a password, for the bench account only.
printf '%s ALL=(ALL) NOPASSWD: ALL\n' "$U" > /etc/sudoers.d/oblivio-bench-claude
chmod 0440 /etc/sudoers.d/oblivio-bench-claude
visudo -cf /etc/sudoers.d/oblivio-bench-claude

# The disphas helper starts without an authentication prompt for that account.
cat > /etc/polkit-1/rules.d/49-oblivio-bench.rules <<'RULES'
polkit.addRule(function(action, subject) {
  if (action.id == "org.disphas.helper" && subject.user == "goupil") {
    return polkit.Result.YES;
  }
});
RULES

# Keyboard and mouse input to the interface over SSH (X11 display :0).
apt-get install -y xdotool

echo "disphas bench ready for $U"
