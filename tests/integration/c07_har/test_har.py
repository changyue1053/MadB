import gzip, io, tarfile, unittest
from unittest.mock import patch
from adapters.artifacts.arkts import har

def make(entries=None, mtime=1, reverse=False):
    entries = entries if entries is not None else [('package/oh-package.json5', b'{"name":"example"}'), ('package/libs', None), ('package/libs/liba.so', b'\x00ELF test bytes')]
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode='w', format=tarfile.PAX_FORMAT) as archive:
        for name, data in reversed(entries) if reverse else entries:
            item = tarfile.TarInfo(name)
            item.mtime = mtime
            item.uid = 42
            item.gid = 17
            item.uname = 'builder'
            item.gname = 'staff'
            item.mode = 493 if data is None else 420
            item.type = tarfile.DIRTYPE if data is None else tarfile.REGTYPE
            item.size = 0 if data is None else len(data)
            archive.addfile(item, None if data is None else io.BytesIO(data))
    return gzip.compress(out.getvalue(), mtime=mtime)

class HarTests(unittest.TestCase):

    def test_different_time_order_and_users_normalize_without_content_change(self):
        a = make()
        b = make(mtime=12345, reverse=True)
        self.assertNotEqual(a, b)
        self.assertEqual(har.canonicalize(a), har.canonicalize(b))
        self.assertEqual(har.read_members(a), har.read_members(har.canonicalize(a)))

    def test_idempotent(self):
        a = har.canonicalize(make())
        self.assertEqual(a, har.canonicalize(a))

    def test_explicit_root_is_supported(self):
        a = make([('package', None), ('package/oh-package.json5', b'{}')])
        self.assertEqual(len(har.read_members(a)), 2)

    def test_regular_permissions_preserved(self):
        a = har.canonicalize(make())
        self.assertEqual({m.path: m.mode for m in har.read_members(a)}, {'package/oh-package.json5': 420, 'package/libs': 493, 'package/libs/liba.so': 420})

    def test_path_rejection(self):
        for name in ['../escape', '/package/escape', 'package/../escape', 'other/x', 'package/./x', 'package//x', 'package\\x']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                har.canonicalize(make([(name, b'x')]))

    def test_case_and_unicode_aliases_rejected(self):
        for a, b in [('x', 'X'), ('é', 'é')]:
            with self.subTest(a=a, b=b), self.assertRaisesRegex(ValueError, 'alias'):
                har.canonicalize(make([('package/oh-package.json5', b'{}'), ('package/' + a, b'first'), ('package/' + b, b'last')]))

    def test_control_characters_rejected(self):
        for char in ['\n', '\t', '\r', '\x7f', '\x85']:
            with self.subTest(char=char), self.assertRaisesRegex(ValueError, 'path'):
                har.canonicalize(make([('package/oh-package.json5', b'{}'), ('package/a' + char + 'x', b'content')]))

    def test_duplicate(self):
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            har.canonicalize(make([('package/oh-package.json5', b'{}')] * 2))

    def test_missing_manifest(self):
        with self.assertRaisesRegex(ValueError, 'manifest'):
            har.canonicalize(make([('package/x', b'x')]))

    def test_root_cannot_be_file(self):
        with self.assertRaisesRegex(ValueError, 'root'):
            har.canonicalize(make([('package', b'x'), ('package/oh-package.json5', b'{}')]))

    def test_file_parent(self):
        with self.assertRaisesRegex(ValueError, 'parent'):
            har.canonicalize(make([('package/oh-package.json5', b'{}'), ('package/p', b'x'), ('package/p/x', b'x')]))

    def test_missing_nested_parent(self):
        with self.assertRaisesRegex(ValueError, 'parent'):
            har.canonicalize(make([('package/oh-package.json5', b'{}'), ('package/p/x', b'x')]))

    def changed_header(self, change):
        out = io.BytesIO()
        with tarfile.open(fileobj=out, mode='w', format=tarfile.PAX_FORMAT) as archive:
            item = tarfile.TarInfo('package/oh-package.json5')
            item.mode = 420
            change(item)
            archive.addfile(item)
        return gzip.compress(out.getvalue(), mtime=0)

    def test_links_devices_fifo_rejected(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.CHRTYPE, tarfile.BLKTYPE, tarfile.FIFOTYPE]:

            def change(item):
                item.type = kind
                item.linkname = 'outside' if kind in [tarfile.SYMTYPE, tarfile.LNKTYPE] else ''
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                har.canonicalize(self.changed_header(change))

    def test_special_modes_rejected(self):
        with self.assertRaisesRegex(ValueError, 'permission'):
            har.canonicalize(self.changed_header(lambda item: setattr(item, 'mode', 2541)))

    def test_unknown_pax_rejected(self):
        with self.assertRaisesRegex(ValueError, 'PAX'):
            har.canonicalize(self.changed_header(lambda item: setattr(item, 'pax_headers', {'unknown.semantic': '1'})))

    def test_known_pax_metadata_normalized(self):
        a = self.changed_header(lambda item: setattr(item, 'pax_headers', {'mtime': '123.456', 'uname': 'someone'}))
        self.assertEqual(har.read_members(a), har.read_members(har.canonicalize(a)))

    def test_archive_budget(self):
        with patch.object(har, 'MAX_ARCHIVE_BYTES', 1), self.assertRaisesRegex(ValueError, 'archive exceeds'):
            har.canonicalize(make())

    def test_expanded_budget(self):
        with patch.object(har, 'MAX_EXPANDED_BYTES', 1024), self.assertRaisesRegex(ValueError, 'expanded'):
            har.canonicalize(make())

    def test_member_budget(self):
        with patch.object(har, 'MAX_MEMBER_BYTES', 2), self.assertRaisesRegex(ValueError, 'member size'):
            har.canonicalize(make())

    def test_member_count(self):
        with patch.object(har, 'MAX_MEMBERS', 1), self.assertRaisesRegex(ValueError, 'member count'):
            har.canonicalize(make())

    def test_recursive_extended_headers_rejected(self):
        payload = b'11 mtime=1\n'
        info = tarfile.TarInfo('PaxHeaders/time')
        info.type = tarfile.XHDTYPE
        info.size = len(payload)
        extended = info.tobuf(format=tarfile.USTAR_FORMAT) + payload.ljust(512, b'\0')
        data = gzip.compress(extended * 1100 + gzip.decompress(make()), mtime=0)
        with self.assertRaisesRegex(ValueError, 'invalid HAR tar'):
            har.canonicalize(data)

    def test_trailing_data_rejected(self):
        data = gzip.compress(gzip.decompress(make()) + b'unaccounted', mtime=0)
        with self.assertRaisesRegex(ValueError, 'trailing'):
            har.canonicalize(data)

    def test_invalid_gzip(self):
        with self.assertRaisesRegex(ValueError, 'gzip'):
            har.canonicalize(b'not an archive')

    def test_invalid_tar(self):
        with self.assertRaisesRegex(ValueError, 'tar'):
            har.canonicalize(gzip.compress(b'not tar'))

    def test_corrupt_deflate(self):
        data = bytearray(make())
        data[20] ^= 255
        with self.assertRaisesRegex(ValueError, 'gzip'):
            har.canonicalize(bytes(data))

    def test_truncated_gzip(self):
        with self.assertRaises(ValueError):
            har.canonicalize(make()[:-8])

    def test_inventory_does_not_infer_sdk_acceptance(self):
        files = har.inventory(make())
        self.assertEqual(len(files), 2)
        self.assertEqual(set(files[0]), {'path', 'size', 'sha256', 'mode'})
if __name__ == '__main__':
    unittest.main()
