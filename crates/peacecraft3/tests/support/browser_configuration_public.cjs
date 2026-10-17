const path = require('node:path');
if (!process.argv[2]) throw new Error('usage: node browser_configuration_public.cjs <generated map_loading JS>');
const actual = require(path.resolve(process.argv[2]));
if (actual.browser_configuration_json_gate() !== 7) throw new Error('JSON group count');
if (actual.browser_configuration_consumer_gate() !== 7) throw new Error('consumer group count');
console.log('7 JSON + 7 public App consumer groups passed');
