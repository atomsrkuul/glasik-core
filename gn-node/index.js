// index.js - GN Native Node.js bindings
// Auto-selects the correct native binary for the current platform/arch
const { join } = require('path');
const binding = require(join(__dirname, `gn-native.${process.platform}-${process.arch}-gnu.node`));
module.exports = binding;
