#!/usr/bin/env node

// Reuse Mint's native binary discovery, argument forwarding, and exit handling.
process.argv.splice(2, 0, 'web');
require('./index.js');
