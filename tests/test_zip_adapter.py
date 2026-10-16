import importlib.util
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('check_zip', ROOT/'scripts/check_zip.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ZipAdapterTests(unittest.TestCase):
    def test_bash_unzip_extracts_unicode_in_new_folder_without_overwriting(self):
        with tempfile.TemporaryDirectory() as d:
            archive = Path(d)/"a ' & 中文.zip"
            with zipfile.ZipFile(archive, 'w') as z:
                z.writestr('folder/中文.txt', 'hello')
            for _ in range(2):
                subprocess.run(['bash', str(ROOT/'scripts/unzip.sh'), str(archive)], check=True, capture_output=True)
            outputs = list(Path(d).glob('*.unpacked.*'))
            self.assertEqual(len(outputs), 2)
            self.assertEqual((outputs[0]/'folder/中文.txt').read_text(), 'hello')
            self.assertTrue(archive.exists())

    def test_rejects_traversal_absolute_paths_and_symlinks_before_extraction(self):
        with tempfile.TemporaryDirectory() as d:
            archive = Path(d)/'unsafe.zip'
            for name in ['../escape', '/absolute', 'C:/drive', 'a/../../escape', 'a\\..\\escape']:
                with zipfile.ZipFile(archive, 'w') as z: z.writestr(name, 'bad')
                with self.assertRaises(ValueError): module.check(archive)
            with zipfile.ZipFile(archive, 'w') as z:
                item = zipfile.ZipInfo('link'); item.create_system = 3; item.external_attr = (stat.S_IFLNK | 0o777) << 16
                z.writestr(item, '/tmp')
            with self.assertRaises(ValueError): module.check(archive)
