// scripts/before-build.js
const { execSync } = require('child_process');

module.exports = async function () {
  console.log('Running native rebuild...');
  execSync('node scripts/rebuild-native.js', { stdio: 'inherit' });
};
