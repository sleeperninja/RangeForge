const { spawnSync } = require('child_process');
const os = require('os');
const path = require('path');
const fs = require('fs');

const chalk = require('chalk');

console.log(chalk.cyan('🖥️ Platform detected:'), os.platform());

if (os.platform() === 'win32') {
  console.log(chalk.yellow('🪟 Windows detected'));

  // Set GYP_MSVS_VERSION for node-gyp
  process.env.GYP_MSVS_VERSION = '2022';

  // Try to locate Visual Studio vcvarsall.bat
  const vsPaths = [
    process.env.VSINSTALLDIR ? path.join(process.env.VSINSTALLDIR, 'VC', 'Auxiliary', 'Build', 'vcvarsall.bat') : null,
    'C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat',
    'C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat',
  ].filter(Boolean);

  let vcvarsPath = vsPaths.find(p => fs.existsSync(p));
  if (!vcvarsPath) {
    console.error(chalk.red('❌ Could not locate vcvarsall.bat for Visual Studio 2022.'));
    console.error('Please ensure VS Build Tools 2022 is installed.');
    process.exit(1);
  }

  // Wrap path in quotes to handle spaces
  vcvarsPath = `"${vcvarsPath}"`;

  const command = `${vcvarsPath} x64 && npx electron-rebuild -f -w mmap-io`;

  console.log(chalk.green('🔧 Running:'), command);

  const result = spawnSync(command, { shell: true, stdio: 'inherit', env: process.env });

  if (result.status !== 0) {
    console.error(chalk.red('❌ Rebuild failed'));
    process.exit(result.status);
  } else {
    console.log(chalk.green('✅ Rebuild complete!'));
  }

} else {
  console.log(chalk.yellow('💻 Non-Windows platform detected, running electron-rebuild directly'));

  const result = spawnSync('npx', ['electron-rebuild', '-f', '-w', 'mmap-io'], { stdio: 'inherit', env: process.env });

  if (result.status !== 0) {
    console.error(chalk.red('❌ Rebuild failed'));
    process.exit(result.status);
  } else {
    console.log(chalk.green('✅ Rebuild complete!'));
  }
}
