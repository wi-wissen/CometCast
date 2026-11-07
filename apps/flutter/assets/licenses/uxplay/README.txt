Where a build ships UxPlay (AirPlay receiving), its packaging copies the licence file here as
UXPLAY_LICENSE.txt: the GPL-3.0 text of UxPlay followed by the notices of its bundled parts.
The app then lists "UxPlay" second in the licence list. A build without UxPlay leaves this
folder as it is. See tools/gen-third-party.mjs (--uxplay-licence) and tools/release-guard.sh
(uxplay-licence).
