#!/usr/bin/env python3
"""Checks that every automated test of docs/PRUEBAS.md exists.

A test declares what it covers with a comment line `// covers: U-04, RF-06`
right above it (Rust and TypeScript). This script reads the test tables of
PRUEBAS.md (U, PT, I and D rows) and the requirement matrix (section 4) and
fails when:

  * an automated test ID has no test that covers it, or
  * a requirement of the matrix lists automated IDs and none of them is covered.

Manual (M-*) and interface (UI) rows are out of its reach on purpose: they are
checked by people and by the component tests in ui/.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC = (ROOT / "docs" / "PRUEBAS.md").read_text(encoding="utf-8")
ID = re.compile(r"\b(?:U|PT|I|D)-\d{2}\b")
RETIRED = {"U-12", "I-12"}  # ADR-023: kept as a replaced row; I-12 is covered by a test that says so.

# Tests may live in any Rust or TypeScript source file of the repository.
covered: set[str] = set()
for path in list(ROOT.glob("crates/**/*.rs")) + list(ROOT.glob("ui/src/**/*.ts*")) + list(ROOT.glob("tools/*.sh")):
    if "node_modules" in path.parts or "target" in path.parts:
        continue
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        match = re.search(r"(?://|#)\s*covers:\s*(.+)$", line)
        if match:
            covered.update(re.findall(r"[A-Z]+-\d+", match.group(1)))

# Test definitions: the first column of the tables in section 2.
defined: set[str] = set()
section2 = DOC.split("## 2. Niveles", 1)[1].split("## 3.", 1)[0]
for row in section2.splitlines():
    cell = row.split("|")[1].strip() if row.startswith("|") else ""
    if ID.fullmatch(cell):
        defined.add(cell)
defined -= {"U-12"}
# I-12 is retired but replaced by a check that `--replace` is invalid.
missing = sorted(i for i in defined if i not in covered)

# Requirements: a row of the section 4 matrix.
unmet = []
section4 = DOC.split("## 4. Matriz de requisitos a pruebas", 1)[1].split("## 5.", 1)[0]
for row in section4.splitlines():
    cells = [cell.strip() for cell in row.split("|")[1:-1]] if row.startswith("|") else []
    if len(cells) != 2 or not re.match(r"(RF|NF)-", cells[0]):
        continue
    wanted = set(ID.findall(cells[1])) - RETIRED
    if wanted and not (wanted & covered):
        unmet.append(f"{cells[0]} needs one of {sorted(wanted)}")

if missing or unmet:
    if missing:
        print("tests without a covering test:", ", ".join(missing))
    for line in unmet:
        print("requirement without coverage:", line)
    sys.exit(1)
print(f"ok: {len(defined)} tests and the requirement matrix are covered")
