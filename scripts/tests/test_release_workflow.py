"""Exercise the actual release shell steps with a local GitHub CLI stand-in."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[2]


def release_step(name):
    lines = (ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8').splitlines()
    start = lines.index('      - name: ' + name) + 1
    end = next((i for i in range(start, len(lines)) if lines[i].startswith('      - ')), len(lines))
    for i in range(start, end):
        if lines[i] == '        run: |':
            return textwrap.dedent('\n'.join(lines[i + 1:end]))
        if lines[i].startswith('        run: '):
            return lines[i].removeprefix('        run: ')
    raise ValueError('missing shell command: ' + name)


GH_STUB = '''\
import json
from pathlib import Path
import sys

args = sys.argv[1:]
path = Path('github-release.json')
state = json.loads(path.read_text()) if path.exists() else None
action = args[1]
if action == 'view':
    if state is None:
        sys.exit(1)
    print(json.dumps(state))
    sys.exit(0)
if action == 'create':
    if state is not None:
        sys.exit('release already exists')
    state = {'isDraft': False, 'isPrerelease': False, 'isLatest': False,
             'title': args[args.index('--title') + 1],
             'body': Path(args[args.index('--notes-file') + 1]).read_text(encoding='utf-8')}
elif action != 'edit' or state is None:
    sys.exit('unexpected GitHub command')
for flag, field in [('--draft', 'isDraft'), ('--prerelease', 'isPrerelease'), ('--latest', 'isLatest')]:
    for arg in args:
        if arg == flag or arg.startswith(flag + '='):
            state[field] = arg != flag + '=false'
path.write_text(json.dumps(state), encoding='utf-8')
'''


@unittest.skipUnless(os.name == 'posix' and shutil.which('bash'), 'release shell integration needs POSIX bash')
class ReleaseWorkflow(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.state_path = self.root / 'github-release.json'
        self.stub = self.root / 'gh_stub.py'
        self.stub.write_text(GH_STUB, encoding='utf-8')

    def run_step(self, name, product='joyword'):
        env = dict(os.environ, GH_TEST_PYTHON=sys.executable, GH_TEST_STUB=str(self.stub),
                   RELEASE_PRODUCT=product, RELEASE_VERSION='0.3.0', RELEASE_TAG=product + '-v0.3.0',
                   GITHUB_REPOSITORY='example/joyoffice')
        # A shell function makes it impossible for these steps to call a real gh.
        script = 'gh() { "$GH_TEST_PYTHON" "$GH_TEST_STUB" "$@"; }\n' + release_step(name)
        return subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', script], cwd=self.root,
                              env=env, text=True, encoding='utf-8', capture_output=True)

    def notes(self, product='joyword'):
        path = self.root / 'docs/releases' / (product + '-v0.3.0.md')
        path.parent.mkdir(parents=True, exist_ok=True)
        body = (ROOT / 'docs/releases' / path.name).read_text(encoding='utf-8')
        path.write_text(body, encoding='utf-8')
        return body

    def test_each_product_creates_resumes_and_publishes_a_normal_release(self):
        for product, title in [('joyword', 'JoyWord'), ('joydeck', 'JoyDeck'), ('joysheet', 'JoySheet')]:
            with self.subTest(product=product):
                self.state_path.unlink(missing_ok=True)
                self.notes(product)
                for _ in range(2):
                    result = self.run_step('Create or resume a curated release draft', product)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    state = json.loads(self.state_path.read_text())
                    self.assertTrue(state['isDraft'])
                    self.assertFalse(state['isPrerelease'])
                    self.assertFalse(state['isLatest'])
                    self.assertEqual(state['title'], title + ' v0.3.0')
                result = self.run_step('Publish completed release', product)
                self.assertEqual(result.returncode, 0, result.stderr)
                state = json.loads(self.state_path.read_text())
                self.assertFalse(state['isDraft'])
                self.assertFalse(state['isPrerelease'])
                self.assertTrue(state['isLatest'])

    def test_published_prerelease_or_mismatched_notes_are_refused_without_mutation(self):
        notes = self.notes()
        for changes in [{'isDraft': False}, {'isPrerelease': True}, {'isPrerelease': None},
                        {'body': notes + '\nUnexpected change'}]:
            state = dict(isDraft=True, isPrerelease=False, body=notes, title='JoyWord v0.3.0')
            state.update(changes)
            with self.subTest(changes=list(changes)):
                self.state_path.write_text(json.dumps(state), encoding='utf-8')
                result = self.run_step('Create or resume a curated release draft')
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(json.loads(self.state_path.read_text()), state)
