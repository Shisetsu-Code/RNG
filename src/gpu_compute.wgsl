// wgpu WGSL: una invocacion analiza los prefijos de un paquete de 100.
// NO genera aleatoriedad: solo calcula el RTP condicional a partir de
// los resultados previos y del catalogo publico. Todos son enteros exactos.
@group(0) @binding(0) var<storage, read> ids: array<u32>;
@group(0) @binding(1) var<storage, read> payouts: array<u32>;
@group(0) @binding(2) var<storage, read_write> expected: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch_index = gid.x;
    let offset = batch_index * 100u;
    if offset >= arrayLength(&ids) {
        return;
    }

    var remaining: u32 = 0u;
    for (var j: u32 = 0u; j < 128u; j = j + 1u) {
        remaining = remaining + payouts[j];
    }
    var count = 128u;

    for (var k: u32 = 0u; k < 100u; k = k + 1u) {
        if (k == 0u)  { expected[batch_index * 6u + 0u] = remaining / count; }
        if (k == 25u) { expected[batch_index * 6u + 1u] = remaining / count; }
        if (k == 50u) { expected[batch_index * 6u + 2u] = remaining / count; }
        if (k == 75u) { expected[batch_index * 6u + 3u] = remaining / count; }
        if (k == 90u) { expected[batch_index * 6u + 4u] = remaining / count; }
        if (k == 99u) { expected[batch_index * 6u + 5u] = remaining / count; }

        let id = ids[offset + k];
        remaining = remaining - payouts[id - 1u];
        count = count - 1u;
    }
}
