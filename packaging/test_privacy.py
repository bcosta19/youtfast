import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import privacy
from release import ROOT


class PrivacyTests(unittest.TestCase):
    def test_findings_never_include_the_matching_credential(self):
        value = ("ghp_" + "a" * 36).encode()
        actual = privacy.findings(b"synthetic " + value)
        self.assertEqual(actual, {"github_credential": 1})
        self.assertNotIn(value.decode(), str(actual))

    def test_history_never_reads_a_known_report_even_if_it_has_another_name(self):
        report = "a" * 40
        code = "b" * 40
        calls = []

        def git(*args, **_kwargs):
            calls.append(args)
            if args[0] == "rev-list" and "--objects" in args:
                return f"{report} rustypipe_reports/private.json\n{code} src/main.rs\n".encode()
            if args[0] == "cat-file" and "--batch-all-objects" in args:
                return f"{report} blob 12\n{code} blob 5\n".encode()
            if args[:2] == ("cat-file", "blob"):
                self.assertNotEqual(args[2], report)
                return b"code\n"
            if args[0] == "rev-list":
                return b"2\n"
            return b"refs/heads/main\n"

        with patch.object(privacy, "git", side_effect=git):
            result = privacy.history()
        self.assertEqual(result["excluded_sensitive_object_ids"], [report])
        self.assertEqual(result["scanned_blobs"], 1)
        self.assertEqual(result["credential_pattern_findings"], [])

    def test_tree_rejects_report_paths_before_opening_them(self):
        with self.assertRaises(ValueError):
            privacy.working_tree(["rustypipe_reports/private.json"])

    def test_tree_manifest_does_not_collect_unlisted_local_files(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="privacy-test-", dir=ROOT / "target") as temporary:
            root = Path(temporary)
            (root / "public.md").write_text("Public documentation")
            (root / "unlisted.txt").write_text("Local work")
            report = privacy.working_tree(["public.md"], root)
            self.assertEqual([item["path"] for item in report["files"]], ["public.md"])
            self.assertEqual(report["credential_pattern_findings"], {})


if __name__ == "__main__":
    unittest.main()
