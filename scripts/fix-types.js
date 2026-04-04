// Post-build script: fix NAPI-RS generated index.d.ts
// NAPI-RS generates single-arg callbacks but CalleeHandled mode uses (err, result) at runtime
const fs = require('fs');
const path = require('path');

const file = path.join(__dirname, '..', 'index.d.ts');
let content = fs.readFileSync(file, 'utf-8');

// Fix callback signatures to include err parameter
content = content.replace(
  /callback: \(event: QoreEvent\) => void/g,
  'callback: (err: Error | null, event: QoreEvent) => void'
);

fs.writeFileSync(file, content);
console.log('✅ Fixed index.d.ts callback signatures');
