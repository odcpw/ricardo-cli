#!/usr/bin/env python3
"""Validate package structure and execute the meaningful allocation/gate regressions."""
import ast
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

# Port hygiene: provenance anchors and legacy-marketplace vocabulary must not return.
LEGACY_TERMS = ('eb' + 'ay', '§', 'corpus/', 'quote_bank', 'triangulated_kernel', 'operator_library')
NEWER_SELLING = {'sold', 'drafts', 'status'}
TESTS = ('test-project.py', 'test-assistant.py', 'test-monitor.py', 'test-draft.py', 'test-pickup.py')


def heading_slugs(doc):
    slugs = set()
    for line in doc.read_text().splitlines():
        match = re.match(r'#{1,6}\s+(.*?)\s*$', line)
        if match:
            title = re.sub(r'[`*_]', '', match.group(1)).lower()
            slugs.add(re.sub(r'\s', '-', re.sub(r'[^\w\s-]', '', title)))
    return slugs


def link_targets(doc):
    return [target for target in re.findall(r'\]\(([^)\s]+)\)', doc.read_text())]


def main():
    root = Path(__file__).resolve().parents[1]
    text = (root / 'SKILL.md').read_text()
    errors = []
    if not text.startswith('---\n') or text.count('\n---\n') != 1:
        errors.append('invalid frontmatter boundaries')
    front = text.split('---', 2)[1]
    if not re.search(r'^name: sell-my-stuff-on-ricardo$', front, re.M):
        errors.append('frontmatter name must be sell-my-stuff-on-ricardo')
    desc = front.split('description: >-', 1)[-1].strip().replace('\n  ', ' ')
    if len(desc) > 400 or 'Use when' not in desc:
        errors.append('description must be under 400 characters and contain Use when')
    if len(text.splitlines()) > 250:
        errors.append('core skill exceeds 250 lines')

    docs = [root / 'SKILL.md', *sorted((root / 'references').glob('*.md')), *sorted((root / 'assets').glob('*.md'))]
    for doc in docs:
        for target in link_targets(doc):
            if target.startswith(('https://', 'http://')):
                continue
            path, _, fragment = target.partition('#')
            destination = (doc.parent / path).resolve() if path else doc
            if not destination.is_file():
                errors.append(f'broken reference in {doc.relative_to(root)}: {target}')
            elif fragment and destination.suffix == '.md' and fragment not in heading_slugs(destination):
                errors.append(f'broken anchor in {doc.relative_to(root)}: {target}')

    # Every script named in the docs must exist and be executable.
    scripts = {path.name for path in (root / 'scripts').glob('*.py')}
    for doc in docs:
        for name in set(re.findall(r'(?:\./)?scripts/([a-z][a-z-]*\.py)', doc.read_text())):
            if name not in scripts:
                errors.append(f'{doc.relative_to(root)} names missing script {name}')

    for file in root.rglob('*'):
        if file.is_symlink():
            errors.append(f'packaged symlink: {file.relative_to(root)}')
        if file.is_file() and file.suffix == '.py':
            source = file.read_text()
            ast.parse(source, filename=str(file))
            if not os.access(file, os.X_OK) or not source.startswith('#!/usr/bin/env python3'):
                errors.append(f'nonexecutable/missing shebang: {file.name}')
        # RICARDO-PLATFORM.md is a separately maintained, sourced research file and may cite other marketplaces.
        if (file.is_file() and file.suffix in {'.md', '.py', '.json'}
                and file.name not in {Path(__file__).name, 'RICARDO-PLATFORM.md'}):
            lowered = file.read_text().lower()
            for term in LEGACY_TERMS:
                if term in lowered:
                    errors.append(f'{file.relative_to(root)}: legacy term {term!r}')

    # When the ricardo binary is installed, documented commands must exist in its local capability list.
    binary = shutil.which('ricardo')
    if binary:
        try:
            result = subprocess.run([binary, 'capabilities', '--json'], capture_output=True, text=True,
                                    check=False, timeout=15)
            listed = {row['command'] for row in json.loads(result.stdout)['data']['commands']}
        except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
            listed = None
            errors.append('ricardo capabilities --json did not return a command list')
        if listed:
            for doc in (root / 'SKILL.md', root / 'references/RICARDO-CLI.md'):
                if not doc.is_file():
                    continue
                for line in doc.read_text().splitlines():
                    for match in re.findall(r'`ricardo ([a-z][a-z-]*(?: [a-z][a-z-]*)?)', line):
                        words = match.split()
                        two = ' '.join(words[:2])
                        if words[0] == 'selling' and len(words) == 2 and words[1] in NEWER_SELLING:
                            # Read-only seller commands that only newer CLI versions list.
                            if two not in listed and 'if listed' not in line.lower():
                                errors.append(f'{doc.relative_to(root)}: unlisted command needs an "if listed" marker: ricardo {match}')
                        elif two not in listed and words[0] not in listed:
                            errors.append(f'{doc.relative_to(root)}: command not in ricardo capabilities: ricardo {match}')
    else:
        print('SKIP: ricardo binary not on PATH; documented commands were not compared with capabilities')

    for error in errors:
        print(f'FAIL: {error}')
    if errors:
        raise SystemExit(1)
    for name in TESTS:
        result = subprocess.run([sys.executable, '-B', str(root / 'scripts' / name)], check=False, timeout=60)
        if result.returncode:
            raise SystemExit(result.returncode)
    print('PASS: package links, frontmatter, source syntax, executable bits and regression suites')


if __name__ == '__main__':
    main()
