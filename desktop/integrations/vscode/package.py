#!/usr/bin/env python3
"""Build the local adapter without npm downloads or marketplace access."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile


def package(root):
    metadata = json.loads((root / 'package.json').read_text())
    for key in ('name', 'publisher'):
        if not re.fullmatch(r'[a-z0-9-]+', metadata[key]):
            raise ValueError('invalid extension identity')
    if not re.fullmatch(r'\d+\.\d+\.\d+', metadata['version']):
        raise ValueError('invalid extension version')
    manifest = ET.Element('PackageManifest', {'Version': '2.0.0', 'xmlns': 'http://schemas.microsoft.com/developer/vsx-schema/2011'})
    info = ET.SubElement(manifest, 'Metadata')
    ET.SubElement(info, 'Identity', {'Language': 'en-US', 'Id': metadata['name'], 'Version': metadata['version'], 'Publisher': metadata['publisher']})
    ET.SubElement(info, 'DisplayName').text = metadata['displayName']
    ET.SubElement(info, 'Description', {'{http://www.w3.org/XML/1998/namespace}space': 'preserve'}).text = metadata['description']
    ET.SubElement(info, 'Categories').text = 'Other'
    ET.SubElement(info, 'License').text = 'extension/LICENSE'
    properties = ET.SubElement(info, 'Properties')
    ET.SubElement(properties, 'Property', {'Id': 'Microsoft.VisualStudio.Code.Engine', 'Value': metadata['engines']['vscode']})
    installation = ET.SubElement(manifest, 'Installation')
    ET.SubElement(installation, 'InstallationTarget', {'Id': 'Microsoft.VisualStudio.Code'})
    ET.SubElement(manifest, 'Dependencies')
    assets = ET.SubElement(manifest, 'Assets')
    ET.SubElement(assets, 'Asset', {'Type': 'Microsoft.VisualStudio.Code.Manifest', 'Path': 'extension/package.json', 'Addressable': 'true'})
    entries = {
        'extension.vsixmanifest': ET.tostring(manifest, encoding='utf-8', xml_declaration=True),
        '[Content_Types].xml': b'<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="json" ContentType="application/json"/><Default Extension="cjs" ContentType="application/javascript"/><Default Extension="md" ContentType="text/markdown"/><Default Extension="vsixmanifest" ContentType="text/xml"/></Types>',
    }
    for name in ('package.json', 'client.cjs', 'extension.cjs', 'README.md', 'LICENSE'):
        entries['extension/' + name] = (root / name).read_bytes()
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(entries.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = 0o100644 << 16
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)
    raw = output.getvalue()
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        assert archive.testzip() is None
        assert set(archive.namelist()) == set(entries)
        for name, data in entries.items():
            assert archive.read(name) == data
        ET.fromstring(archive.read('extension.vsixmanifest'))
    return raw


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--check', action='store_true', help='validate an in-memory package')
    action.add_argument('--output', type=Path, help='create a new VSIX; existing files are refused')
    args = parser.parse_args()
    raw = package(Path(__file__).resolve().parent)
    if args.output:
        if args.output.suffix != '.vsix':
            parser.error('output must end in .vsix')
        with args.output.open('xb') as output:
            output.write(raw)
    print(json.dumps({'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(), 'packageVerified': True}))
