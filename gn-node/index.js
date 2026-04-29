// gni-compression 3.1.0
"use strict";
const path = require("path");
const fs = require("fs");
const binding = require(path.join(__dirname, `gn-native.${process.platform}-${process.arch}-gnu.node`));

const snapPath = path.join(__dirname, "gn-l0-multicorpus.snapshot");
if (fs.existsSync(snapPath)) {
  binding.gnLoadSnapshot(snapPath).catch(() => {});
}

class GNCompressor {
  async compress(data) {
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(String(data));
    return binding.gnCompress(buf);
  }
  async decompress(compressed) {
    return binding.gnDecompress(compressed);
  }
}

const gnCompress = binding.gnCompress;
const gnDecompress = binding.gnDecompress;
const gnCompressAc = binding.gnCompressAc;

module.exports = { GNCompressor, gnCompress, gnDecompress, gnCompressAc, native: binding };
