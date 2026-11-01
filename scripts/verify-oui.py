"""Read-only verification of the four MAC vendor prefixes against IEEE MA-L."""
import urllib.request
prefixes = ('F0-D1-A9', '28-6D-97', '3C-6A-A7', 'B8-27-EB')
with urllib.request.urlopen('https://standards-oui.ieee.org/oui/oui.txt', timeout=45) as response:
    for raw in response:
        line = raw.decode('utf-8', errors='replace')
        if line.startswith(prefixes):
            print(line.strip())
