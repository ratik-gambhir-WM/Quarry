"""Retain the public-domain SQLite unicode61 tables without linking SQLite at runtime.

Usage: python3 tools/generate_unicode61.py /path/to/sqlite3.c
The input must contain the FTS5 Unicode 6.1 routines. Run deliberately when reviewing
lexical compatibility; this is not a build/startup step.
"""
import ast
import bisect
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text()
start = source.index('static int fts5_remove_diacritic(int c, int bComplex){')
source = source[start:source.index('static void sqlite3Fts5UnicodeAscii', start)]

def numbers(name):
    body = re.search(r'\b' + name + r'\[\] = \{(.*?)\};', source, re.S)[1]
    return [int(n) for n in re.findall(r'\d+', body)]

block, category_map, category_data = map(numbers, ['aFts5UnicodeBlock', 'aFts5UnicodeMap', 'aFts5UnicodeData'])
diacritics, offsets = map(numbers, ['aDia', 'aiOff'])
chars_body = re.search(r'aChar\[\] = \{(.*?)\};', source, re.S)[1]
chars = [ord(ast.literal_eval(v)) for v in re.findall(r"'(?:\\.|[^'])'", chars_body)]
entries = [tuple(map(int, v)) for v in re.findall(r'\{(\d+),\s*(\d+),\s*(\d+)\}', re.search(r'aEntry\[\] = \{(.*?)\};', source, re.S)[1])]
starts = [e[0] for e in entries]

def category(code):
    if code >= 1 << 20:
        return 0
    index = bisect.bisect_right(category_map, code & 65535, block[code >> 16], block[1 + (code >> 16)]) - 1
    if index < block[code >> 16] or (code & 65535) >= category_map[index] + (category_data[index] >> 5):
        return 0
    result = category_data[index] & 31
    return (5 if ((code & 65535) - category_map[index]) & 1 else 9) if result == 30 else result

def fold(code):
    result = code
    if code < 128:
        return code + 32 if 65 <= code <= 90 else code
    if code < 65536:
        first, flags, length = entries[bisect.bisect_right(starts, code) - 1]
        if code < first + length and not (1 & flags & (first ^ code)):
            result = (code + offsets[flags >> 1]) & 65535
        index = bisect.bisect_right(diacritics, (result << 3) | 7) - 1
        if result <= (diacritics[index] >> 3) + (diacritics[index] & 7):
            result = chars[index] & 127
    elif 66560 <= code < 66600:
        result = code + 40
    return result

ranges, folds = [], []
for code in range(0x110000):
    if 0xD800 <= code <= 0xDFFF:
        continue
    if category(code) in {0, 5, 6, 7, 8, 9, 13, 14, 15, 31}:
        if ranges and ranges[-1][1] + 1 == code:
            ranges[-1] = (ranges[-1][0], code)
        else:
            ranges.append((code, code))
    delta = fold(code) - code
    if delta:
        if folds and folds[-1][1] + 1 == code and folds[-1][2] == delta:
            folds[-1] = (folds[-1][0], code, delta)
        else:
            folds.append((code, code, delta))

def array(name, values, types):
    rows = [', '.join('(' + ','.join(str(n) for n in v) + ')' for v in values[i:i+3]) for i in range(0,len(values),3)]
    return '#[rustfmt::skip]\nconst '+name+': &[('+types+')] = &[\n    '+',\n    '.join(rows)+',\n];\n'

output = '''//! Unicode 6.1 token categories and folding, derived from SQLite FTS5's public-domain
//! unicode61 implementation (remove_diacritics=2). This retains its exact historical
//! behavior, including composed/decomposed accents and simple case-fold exceptions.
//! Generated deliberately with backend/tools/generate_unicode61.py; no SQLite runtime.

pub(super) fn is_token(c:char)->bool {
    let code=u32::from(c);
    let i=TOKEN_RANGES.partition_point(|(start,_)|*start<=code);
    i>0 && code<=TOKEN_RANGES[i-1].1
}
pub(super) fn is_diacritic(c:char)->bool {
    let code=u32::from(c);
    if !(768..=817).contains(&code) {return false;}
    let (mask,bit)=if code<800 {(0x08029fdfu32,code-768)} else {(0x000361f8u32,code-800)};
    mask & (1<<bit)!=0
}
pub(super) fn fold(c:char)->Option<char> {
    let code=u32::from(c);
    let i=FOLD_RANGES.partition_point(|(start,_,_)|*start<=code);
    let folded=if i>0 && code<=FOLD_RANGES[i-1].1 {i64::from(code)+i64::from(FOLD_RANGES[i-1].2)} else {i64::from(code)};
    u32::try_from(folded).ok().filter(|c|*c!=0).and_then(char::from_u32)
}
'''
output += array('TOKEN_RANGES', ranges, 'u32,u32') + array('FOLD_RANGES', folds, 'u32,u32,i32')
Path(__file__).resolve().parents[1].joinpath('src/adapters/postgres/query/unicode61.rs').write_text(output)
print(f'Retained {len(ranges)} token ranges and {len(folds)} folding ranges')
