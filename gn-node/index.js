"use strict";
const path = require("path");
const fs = require("fs");
const zlib = require("zlib");

const native = require("./gn-native.linux-x64-gnu.node");
const DICT = fs.readFileSync(path.join(__dirname, "dict/gcdict.bin"));

async function compress(buf) {
  const [toks, lits] = await native.gnSplitRawV2(buf);
  let cLit;
  try { cLit = zlib.deflateRawSync(lits, {level:6, dictionary:DICT}); }
  catch(e) { cLit = zlib.deflateRawSync(lits, {level:6}); }
  const cTok = zlib.deflateRawSync(toks, {level:6});
  // format: [4 bytes tok len][compressed toks][compressed lits]
  const out = Buffer.allocUnsafe(4 + cTok.length + cLit.length);
  out.writeUInt32LE(cTok.length, 0);
  cTok.copy(out, 4);
  cLit.copy(out, 4 + cTok.length);
  return out;
}

async function decompress(buf) {
  const tokLen = buf.readUInt32LE(0);
  const cTok = buf.slice(4, 4 + tokLen);
  const cLit = buf.slice(4 + tokLen);
  const toks = zlib.inflateRawSync(cTok);
  let lits;
  try { lits = zlib.inflateRawSync(cLit, {dictionary:DICT}); }
  catch(e) { lits = zlib.inflateRawSync(cLit); }
  return Buffer.from(await native.gnMergeRawV2(toks, lits));
}

module.exports = { compress, decompress, native };
