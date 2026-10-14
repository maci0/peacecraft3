const path = require('node:path');
if (!process.argv[2]) throw new Error('usage: node browser_configuration_private.cjs <generated library unit-test JS>');
const actual = require(path.resolve(process.argv[2]));
if (actual.browser_configuration_private_gate() !== 2) throw new Error('private group count');
console.log('2 private library-unit-test lock/exhaustion groups passed');
