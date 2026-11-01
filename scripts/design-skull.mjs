/**
 * Original DEAD//AIR character-cell sculpture.
 * Coordinates describe anatomy, not a random character field. Run with
 * `node scripts/design-skull.mjs` to regenerate the editable .txt layers.
 * Each glyph samples a deliberately lit surface on a fixed 72 × 48 grid.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
const W = 72, H = 48;
const blank = () => Array.from({ length: H }, () => Array(W).fill(' '));
function inside(x, y, points) {
  let result = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const [ax, ay] = points[i], [bx, by] = points[j];
    if ((ay > y) !== (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax) result = !result;
  }
  return result;
}
function paint(grid, polygon, shader) {
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (inside(x + .5, y + .5, polygon)) grid[y][x] = typeof shader === 'function' ? shader(x, y) : shader;
  }
}
const mirror = points => points.map(([x, y]) => [W - x, y]);
const upper = blank(), jaw = blank(), eyes = blank();
const bone = [[33,2],[42,2],[49,3],[55,5],[60,8],[63,12],[64,16],[63,20],[61,23],[65,25],[63,28],[58,30],[53,29],[51,32],[46,33],[26,33],[21,32],[19,29],[14,30],[9,28],[7,25],[11,23],[9,20],[8,16],[9,12],[12,8],[17,5],[23,3]];
const shades = '.:+*#%@';
paint(upper, bone, (x, y) => {
  // Rounded cranial vault: bright frontal bone, shadow at temples and right rim.
  const lighting = 1 - Math.pow((x - 33) / 36, 2) * .67 - Math.max(0, y - 13) * .013 - (x > 50 ? .12 : 0);
  return shades[Math.max(0, Math.min(6, Math.floor(lighting * 6)))];
});
// Frontal highlight and sagittal/frontal seams; sparse cuts follow the bone.
paint(upper, [[24,5],[33,3],[44,4],[49,6],[32,5],[20,10],[17,13],[17,9]], '@');
paint(upper, [[36,3],[38,6],[36,9],[38,11],[37,13],[36,10],[35,8],[37,6]], '+');
paint(upper, [[13,11],[16,9],[15,17],[13,20],[11,17]], ':');
paint(upper, [[59,10],[62,14],[61,20],[58,22],[57,18]], ':');
// Continuous supraorbital ridges, then deep, angular orbital cavities.
const brow = [[15,15],[21,13],[28,14],[34,17],[33,19],[28,17],[20,16],[15,18]];
paint(upper, brow, '@'); paint(upper, mirror(brow), '%');
const socket = [[16,17],[21,16],[28,17],[32,19],[31,23],[27,25],[21,25],[16,23],[14,20]];
paint(upper, socket, ' '); paint(upper, mirror(socket), ' ');
// Recessed rims sit within, not across, the negative-space eye sockets.
const socketRim = [[16,18],[21,17],[27,18],[30,20],[29,21],[26,19],[20,18],[16,20]];
paint(upper, socketRim, '.'); paint(upper, mirror(socketRim), '.');
// Paired nasal opening with a narrow retained septum at its lower edge.
paint(upper, [[35,22],[37,22],[40,27],[39,29],[36,28],[33,29],[32,27]], ' ');
paint(upper, [[36,26],[37,29],[35,29]], ':');
// Zygomatic arches and the hollow between the cheekbone and maxilla.
const cheek = [[9,24],[12,24],[17,27],[22,27],[23,29],[17,29],[12,28]];
paint(upper, cheek, '%'); paint(upper, mirror(cheek), '#');
const hollow = [[12,25],[16,26],[19,28],[15,28]];
paint(upper, hollow, ' '); paint(upper, mirror(hollow), ' ');
paint(upper, [[23,26],[26,27],[28,30],[26,31],[24,29]], ':');
paint(upper, [[49,26],[46,27],[44,30],[46,31],[48,29]], ':');
// Maxilla and eight separately articulated upper teeth; taller central incisors.
paint(upper, [[25,29],[31,30],[41,30],[47,29],[48,32],[44,34],[28,34],[24,32]], '#');
for (let tooth = 0; tooth < 8; tooth++) {
  const x = 24 + tooth * 3, bottom = tooth === 0 || tooth === 7 ? 33 : 34;
  for (let y = 30; y < bottom; y++) { upper[y][x] = tooth < 4 ? '@' : '%'; upper[y][x + 1] = y === bottom - 1 ? '#' : '@'; upper[y][x + 2] = ' '; }
}
// Two small angular embers live inside the sockets. Their layers can dim or
// turn crimson without recoloring the brow, nasal cavity, or cheekbones.
const ember = [[20,20],[23,19],[27,20],[28,22],[25,23],[21,22]];
paint(eyes, ember, (x, y) => y === 21 && x >= 22 && x <= 25 ? '@' : y === 20 ? ':' : '+');
paint(eyes, mirror(ember), (x, y) => y === 21 && x >= 46 && x <= 49 ? '@' : y === 20 ? ':' : '+');
// Mandible is a separate rigid piece. Condylar arms flank the maxilla; the
// entire layer translates down on open poses, leaving the upper skull fixed.
const mandible = [[19,29],[22,30],[23,34],[28,36],[44,36],[49,34],[50,30],[53,29],[53,34],[50,38],[45,40],[27,40],[22,38],[19,34]];
paint(jaw, mandible, (x, y) => y >= 38 ? '+' : x < 35 ? '%' : '#');
paint(jaw, [[23,34],[28,36],[44,36],[49,34],[46,38],[27,38]], '%');
for (let tooth = 0; tooth < 8; tooth++) {
  const x = 24 + tooth * 3;
  // A one-cell dark occlusal seam separates the two rows of teeth at rest.
  jaw[34][x] = ' '; jaw[34][x + 1] = ' '; jaw[34][x + 2] = ' ';
  for (let y = 35; y < 37; y++) { jaw[y][x] = '#'; jaw[y][x + 1] = '%'; jaw[y][x + 2] = y === 35 ? ' ' : '+'; }
}
// Rows at the origin of the mandible must not cover any fixed upper-bone cells.
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) if (upper[y][x] !== ' ') jaw[y][x] = ' ';
mkdirSync(new URL('../src/components/skull/art/', import.meta.url), { recursive: true });
for (const [name, grid] of Object.entries({ upper, jaw, eyes })) writeFileSync(new URL(`../src/components/skull/art/${name}.txt`, import.meta.url), grid.map(row => row.join('')).join('\n') + '\n', 'utf8');
console.log(`Wrote three intentional ${W} × ${H} character-cell layers.`);
