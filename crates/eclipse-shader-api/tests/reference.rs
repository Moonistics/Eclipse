//! The stages' WGSL against the Rust definitions: the preludes of both lines compile, the
//! frame's WGSL layout is the uploaded struct's, 0.1's WGSL is what it always was, and
//! docs/api/shaders-reference.md and shaders-reference-0.2.md are up to date. Rewrite the
//! references with `ECLIPSE_WRITE_DOCS=1 cargo test -p eclipse-shader-api --test reference`.
use eclipse_shader_api::{Api, Frame, Pack, StageKind, compose, package::sha256_hex, reference};
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn validate(text: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(text)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.emit_to_string(text)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}\n{text}", e.emit_to_string(text)));
    module
}

/// Vanilla's look, written as each stage.
fn vanilla(kind: StageKind) -> &'static str {
    match kind {
        StageKind::Sky => "fn sky(input: SkyInput) -> vec4<f32> { return input.color; }\n",
        StageKind::Material => {
            "fn material(input: MaterialInput) -> Surface {\n    \
             return Surface(input.albedo * input.tint * vec4<f32>(input.lightmap, 1.0));\n}\n"
        }
        StageKind::Vertex => {
            "fn vertex(input: VertexInput) -> vec3<f32> { return input.position; }\n"
        }
        StageKind::Weather => {
            "fn weather(input: WeatherInput) -> vec4<f32> { return input.color; }\n"
        }
        StageKind::Post => "fn post(input: PostInput) -> vec4<f32> { return input.color; }\n",
        StageKind::Tonemap => {
            "fn tonemap(input: TonemapInput) -> vec3<f32> { return input.color; }\n"
        }
    }
}

#[test]
fn every_stage_compiles_with_vanilla_s_function() {
    for api in Api::SERVED {
        for &kind in api.stages() {
            let text = format!(
                "{}{}{}",
                kind.prelude(api),
                vanilla(kind),
                kind.entries(api)
            );
            let module = validate(&text);
            assert!(
                module
                    .entry_points
                    .iter()
                    .all(|e| e.name.starts_with("eclipse_")),
                "{api} {kind:?}"
            );
        }
    }
    assert_eq!(Api::V0_1.stages().len(), 5);
    assert!(!Api::V0_1.stages().contains(&StageKind::Tonemap));
}

/// The SHA-256 of 0.1's prelude and entries of each stage, as they were before 0.2: the line
/// packs run on today doesn't move.
const PINNED_0_1: [(StageKind, &str, &str); 5] = [
    (
        StageKind::Sky,
        "3e8217db6c28e4ffe7b3ad09321dc29bb23c14033dedcc21dc3c74308b27bc34",
        "8221c240e49d3759f8d21e0eb36f992ba75f312f26490111082afd424620a0d2",
    ),
    (
        StageKind::Vertex,
        "4e5369684998cf1fdcde38d808f523478a49aa8926c9e2cc9c56964436afbf7c",
        "1a4b4641c7262e853e5ce82f6ae79b79d50dbddefd834a8deebd157ce9d9ba18",
    ),
    (
        StageKind::Material,
        "86377bdb6604cdec3868cb78d7e92fb250e27dc3feedcd148055864c8f302cb4",
        "c516186426f91acb4d6ccc4be2141298ac9f3c9d3e9d1c16cabf7640fec7cabe",
    ),
    (
        StageKind::Weather,
        "85e00bbb2cd702cd76639fe1a316f99f7d270cc932acf44ea1a99eddc1e71a78",
        "2bcd09807ffccf1a9b8491754978820c889be3692724b919c5af999507da1e83",
    ),
    (
        StageKind::Post,
        "01392f68352ffebb15e8d52c5a065effa31328e96ae684f44e348c70f221a110",
        "16398002163c75544ece3b245871f0457fa596e52420211131f3b00336e5510b",
    ),
];

#[test]
fn the_0_1_wgsl_is_unchanged() {
    for (kind, prelude, entries) in PINNED_0_1 {
        let have = (
            sha256_hex(kind.prelude(Api::V0_1).as_bytes()),
            sha256_hex(kind.entries(Api::V0_1).as_bytes()),
        );
        assert_eq!(have, (prelude.to_owned(), entries.to_owned()), "{kind:?}");
    }
}

/// The `Frame` struct the line's sky prelude declares, as (name, offset) and its span.
fn frame_layout(api: Api) -> (Vec<(String, usize)>, usize) {
    let text = format!("{}{}", StageKind::Sky.prelude(api), vanilla(StageKind::Sky));
    let module = validate(&text);
    let (_, ty) = module
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Frame"))
        .expect("Frame in the prelude");
    let naga::TypeInner::Struct { members, span } = &ty.inner else {
        panic!("Frame is a struct")
    };
    let members = members
        .iter()
        .map(|m| (m.name.clone().unwrap(), m.offset as usize))
        .collect();
    (members, *span as usize)
}

#[test]
fn the_frame_s_wgsl_layout_is_the_uploaded_struct_s() {
    let rust: Vec<_> = Frame::WGSL
        .fields
        .iter()
        .zip(Frame::OFFSETS)
        .map(|(f, &o)| (f.name.to_owned(), o))
        .collect();
    let (wgsl, span) = frame_layout(Api::V0_2);
    assert_eq!(span, std::mem::size_of::<Frame>());
    assert_eq!(wgsl, rust);
    // 0.1 declares the start of the same buffer.
    let (wgsl, span) = frame_layout(Api::V0_1);
    assert_eq!(wgsl, rust[..Frame::FIELDS_0_1]);
    assert_eq!(span, rust[Frame::FIELDS_0_1].1);
}

#[test]
fn the_sample_pack_s_stages_compile() {
    let dir = root().join("crates/eclipse-shader-api/tests/samples/schema-1");
    let pack = Pack::read(&dir).unwrap();
    let m = &pack.manifest;
    let stages = [
        (StageKind::Sky, m.sky.iter().collect::<Vec<_>>()),
        (StageKind::Material, m.material.iter().collect()),
        (StageKind::Post, m.post.iter().collect()),
    ];
    for (kind, list) in stages {
        for stage in list {
            let composed = compose(&pack, kind, &stage.file);
            validate(&composed.text);
            // Errors point into the pack's own file.
            let part = composed.parts.iter().find(|(f, _)| *f == stage.file);
            let part = part.unwrap().1.clone();
            let needle = format!("fn {}(", kind.function());
            let at = part.start + composed.text[part].find(&needle).unwrap();
            let source = pack.source(&stage.file).unwrap();
            let line = source.lines().position(|l| l.starts_with(&needle)).unwrap() + 1;
            assert_eq!(composed.locate(at), (stage.file.as_str(), line, 1));
        }
    }
}

#[test]
fn the_references_are_up_to_date() {
    for api in Api::SERVED {
        let doc = reference::path(api);
        let path = root().join(doc);
        let want = reference::markdown(api);
        if std::env::var_os("ECLIPSE_WRITE_DOCS").is_some() {
            std::fs::write(&path, &want).unwrap();
        }
        let have = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            have == want,
            "{doc} is out of date: ECLIPSE_WRITE_DOCS=1 cargo test -p eclipse-shader-api --test reference"
        );
    }
}
