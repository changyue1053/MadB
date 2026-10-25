"""Real pthread/TLS/join on production code; NAPI/libuv are explicit host fixtures."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = Path(__file__).parent / "thread_witness"
NATIVE = ROOT / "sdk/kotlin/arkts-owned/native"


class KotlinThreadWitness(unittest.TestCase):
    def test_real_threads_and_ordinary_removal(self):
        compiler = shutil.which("clang++")
        nm = shutil.which("nm")
        self.assertIsNotNone(compiler, "clang++ is required for the real-thread witness")
        self.assertIsNotNone(nm)
        with tempfile.TemporaryDirectory(prefix="madb-kn-thread-") as directory:
            out = Path(directory)
            common = [compiler, "-std=c++17", "-O2", "-g", "-pthread",
                      "-I" + str(FIXTURE / "include"), "-I" + str(NATIVE)]
            def run(argv, success=True):
                result = subprocess.run(list(map(str, argv)), text=True, capture_output=True, timeout=30)
                if success:
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                else:
                    self.assertNotEqual(result.returncode, 0, "pairwise witness mutant passed")
                return result
            for mode, flags in (("release", []), ("sanitized", ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"])):
                exe = out / mode
                run(common + flags + ["-DMADB_KN_TEST_SUPPORT=1", NATIVE / "executor.cpp", NATIVE / "lifecycle.cpp", FIXTURE / "check.cpp", "-o", exe])
                for case in ("normal", "foreign", "hook-fixture"):
                    result = run([exe, case])
                    self.assertIn("PASS " + case, result.stdout)
            source = (NATIVE / "executor.cpp").read_text()
            marker = "out[9]=simultaneous?!pthread_equal(e->worker,e->cleanupThread):UINT64_MAX;"
            self.assertEqual(source.count(marker), 1)
            mutant = out / "mutant.cpp"
            mutant.write_text(source.replace(marker, "out[9]=simultaneous?0:UINT64_MAX;"))
            exe = out / "mutant"
            run(common + ["-DMADB_KN_TEST_SUPPORT=1", mutant, NATIVE / "lifecycle.cpp", FIXTURE / "check.cpp", "-o", exe])
            failed = run([exe, "normal"], success=False)
            self.assertIn("out[7]==1&&out[8]==1&&out[9]==1", failed.stderr)
            for name in ("executor", "lifecycle"):
                obj = out / (name + ".o")
                run(common + ["-c", NATIVE / (name + ".cpp"), "-o", obj])
                symbols = run([nm, obj]).stdout
                for forbidden in ("test_command", "test_cleanup_release", "testTlsDestructor"):
                    self.assertNotIn(forbidden, symbols)


if __name__ == "__main__":
    unittest.main()
