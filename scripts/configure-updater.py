"""Enable release updater artifacts only with a complete signing configuration."""
import json
import os
from pathlib import Path

public = os.environ.get('HEYDAY_UPDATER_PUBLIC_KEY', '').strip()
private = os.environ.get('TAURI_SIGNING_PRIVATE_KEY', '').strip()
if private and not public:
    raise SystemExit('::error::HEYDAY_UPDATER_PUBLIC_KEY is missing or empty. Add the matching .pub file contents under repository Settings > Secrets and variables > Actions as a repository variable or secret. Environment-scoped settings are not used by this workflow.')
if public and not private:
    raise SystemExit('::error::TAURI_SIGNING_PRIVATE_KEY is missing or empty. Add the matching private key file contents as a repository Actions secret.')
if public:
    path = Path('src-tauri/tauri.conf.json')
    config = json.loads(path.read_text())
    config['bundle']['createUpdaterArtifacts'] = True
    config.setdefault('plugins', {})['updater'] = {'pubkey': public}
    path.write_text(json.dumps(config, indent=2) + '\n')
    print('Signed updater artifacts enabled.')
else:
    print('::warning::Updater keys are not configured. This release supports manual DMG installation only.')
