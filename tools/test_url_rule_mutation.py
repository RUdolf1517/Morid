"""Prove the actual store test rejects swapped URL direction operators."""
from pathlib import Path
import subprocess

path = Path("crates/morid-store/src/data.rs")
original = path.read_bytes()
text = original.decode()
command = ["cargo", "test", "--locked", "-p", "morid-store",
           "url_rule_operators_follow_original_branch_order", "--", "--nocapture"]
try:
    assert text.count("url.ends_with(") == 1
    assert text.count("url.starts_with(") == 1
    mutated = text.replace("url.ends_with(", "url.SWAP(").replace(
        "url.starts_with(", "url.ends_with(").replace("url.SWAP(", "url.starts_with(")
    path.write_text(mutated)
    result = subprocess.run(command, capture_output=True, text=True)
    print("=== ends_with/starts_with SWAPPED ===", flush=True)
    print(result.stdout + result.stderr, flush=True)
    assert result.returncode != 0
    assert "url_rule_operators_follow_original_branch_order ... FAILED" in result.stdout
    assert "suffix: mid-end" in result.stdout + result.stderr
finally:
    path.write_bytes(original)
print("=== original directions RESTORED ===", flush=True)
subprocess.run(command, check=True)
assert path.read_bytes() == original
print("URL_DIRECTION_MUTATION_REJECTED_AND_RESTORED_OK", flush=True)
