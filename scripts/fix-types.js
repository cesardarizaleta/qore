// Post-build script: fix NAPI-RS generated index.d.ts
const fs = require('fs');
const path = require('path');

const file = path.join(__dirname, '..', 'index.d.ts');
if (fs.existsSync(file)) {
  let content = fs.readFileSync(file, 'utf-8');
  content = content.replace(
    /callback: \(event: QoreEvent\) => void/g,
    'callback: (err: Error | null, event: QoreEvent) => void'
  );
  fs.writeFileSync(file, content);
  console.log('✅ Fixed index.d.ts callback signatures');
} else {
  console.log('⚠️ index.d.ts not found for fixing');
}
