// scripts/tree.js
const fs = require('fs');
const path = require('path');

const IGNORE = ['.npm', '.git', 'node_modules', 'mockup_archive', 'mockups','public','release','scripts'];

function printTree(dir, prefix = '') {
  const items = fs.readdirSync(dir).filter(i => !IGNORE.includes(i));
  items.forEach((item, index) => {
    const fullPath = path.join(dir, item);
    const isLast = index === items.length - 1;
    const pointer = isLast ? '└─ ' : '├─ ';
    console.log(prefix + pointer + item);
    if (fs.statSync(fullPath).isDirectory()) {
      printTree(fullPath, prefix + (isLast ? '   ' : '│  '));
    }
  });
}

// usage: node scripts/tree.js electron
const dir = process.argv[2] || '.';
printTree(dir);
