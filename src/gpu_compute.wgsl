// wgpu WGSL: una invocacion analiza prefijos de un paquete de 100.
// Solo analiza matrices ya sorteadas por CPU; no genera ninguna semilla.
// El total del catalogo se precalcula en CPU (128000 premios).
struct CatalogSettings {
    types: u32,
    payout_sum_bps: u32,
    package_size: u32,
    reserved: u32,
}

@group(0) @binding(0) var<storage, read> ids: array<u32>;
@group(0) @binding(1) var<storage, read> payouts: array<u32>;
@group(0) @binding(2) var<storage, read_write> expected: array<u32>;
@group(0) @binding(3) var<uniform> settings: CatalogSettings;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch_index = gid.x;
    let offset = batch_index * settings.package_size;
    if offset >= arrayLength(&ids) {
        return;
    }

    var remaining = settings.payout_sum_bps;
    var possible = settings.types;

    for (var k: u32 = 0u; k < settings.package_size; k = k + 1u) {
        let position = batch_index * 6u;
        if (k == 0u)  { expected[position + 0u] = remaining / possible; }
        if (k == 25u) { expected[position + 1u] = remaining / possible; }
        if (k == 50u) { expected[position + 2u] = remaining / possible; }
        if (k == 75u) { expected[position + 3u] = remaining / possible; }
        if (k == 90u) { expected[position + 4u] = remaining / possible; }
        if (k == 99u) { expected[position + 5u] = remaining / possible; }

        let id = ids[offset + k];
        remaining -= payouts[id - 1u];
        possible -= 1u;
    }
}
