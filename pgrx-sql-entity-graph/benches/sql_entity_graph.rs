//LICENSE Portions Copyright 2019-2021 ZomboDB, LLC.
//LICENSE
//LICENSE Portions Copyright 2021-2023 Technology Concepts & Design, Inc.
//LICENSE Portions Copyright 2023-2023 PgCentral Foundation, Inc. <contact@pgcentral.org>
//LICENSE
//LICENSE All rights reserved.
//LICENSE
//LICENSE Use of this source code is governed by the MIT license that can be found in the LICENSE file.

//! Pure-Rust benchmarks for the SQL entity graph hot paths that run on every
//! extension build: `.control` file parsing, graph construction, and SQL
//! generation. These do not need a PostgreSQL server.
//!
//! Measurement configuration is fixed for reproducibility: sample size 50,
//! warm-up 1s, measurement time 3s. See `perf/baseline.json` at the
//! repository root for the recorded baseline and `perf/run-perf.sh` for the
//! rerun/compare harness.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use pgrx_sql_entity_graph::{
    ControlFile, ExtensionSqlEntity, PgrxSql, SchemaEntity, SqlGraphEntity,
};
use std::hint::black_box;
use std::time::Duration;

const CONTROL_FILE: &str = "\
comment = 'perf bench extension'
default_version = '1.0'
module_pathname = '$libdir/perf_bench'
relocatable = false
superuser = false
schema = 'public'
trusted = false
";

const NUM_SCHEMAS: usize = 4;
const NUM_SQL_ENTITIES: usize = 64;

/// Builds the entity list once per batch so entity construction cost stays
/// out of the timed routine. The entity graph borrows `&str` fields, so the
/// backing strings are deliberately leaked with `Box::leak`; the bench
/// process is short-lived and the leak is bounded by the fixture size.
fn build_fixture_entities() -> Vec<SqlGraphEntity<'static>> {
    fn leak(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }
    let mut schema_names: Vec<&'static str> = Vec::new();
    let mut schema_paths: Vec<&'static str> = Vec::new();
    for i in 0..NUM_SCHEMAS {
        schema_names.push(leak(format!("bench_schema_{i}")));
        schema_paths.push(leak(format!("perf_bench::schema_{i}")));
    }
    let mut sql_names: Vec<&'static str> = Vec::new();
    let mut sql_paths: Vec<&'static str> = Vec::new();
    let mut sql_bodies: Vec<&'static str> = Vec::new();
    for i in 0..NUM_SQL_ENTITIES {
        sql_names.push(leak(format!("bench_sql_{i}")));
        sql_paths.push(leak(format!("perf_bench::sql_{i}")));
        sql_bodies.push(leak(format!("CREATE TABLE bench_table_{i} (id integer, val text);")));
    }

    let control = ControlFile::from_str(CONTROL_FILE).expect("control file parses");
    let mut entities: Vec<SqlGraphEntity<'static>> = vec![SqlGraphEntity::ExtensionRoot(control)];
    for i in 0..NUM_SCHEMAS {
        entities.push(
            SchemaEntity {
                module_path: schema_paths[i],
                name: schema_names[i],
                file: "src/lib.rs",
                line: 1,
            }
            .into(),
        );
    }
    for i in 0..NUM_SQL_ENTITIES {
        entities.push(
            ExtensionSqlEntity {
                module_path: sql_paths[i],
                full_path: sql_paths[i],
                sql: sql_bodies[i],
                file: "src/lib.rs",
                line: 1,
                name: sql_names[i],
                bootstrap: false,
                finalize: false,
                requires: Vec::new(),
                creates: Vec::new(),
            }
            .into(),
        );
    }
    entities
}

fn bench_control_file_parse(c: &mut Criterion) {
    c.bench_function("control_file_parse", |b| {
        b.iter(|| black_box(ControlFile::from_str(black_box(CONTROL_FILE)).unwrap()));
    });
}

fn bench_pgrx_sql_build(c: &mut Criterion) {
    c.bench_function("pgrx_sql_build_64_sql_entities", |b| {
        b.iter_batched(
            build_fixture_entities,
            |entities| {
                black_box(
                    PgrxSql::build(entities.into_iter(), "perf_bench".to_string(), false).unwrap(),
                )
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_pgrx_sql_to_sql(c: &mut Criterion) {
    let entities = build_fixture_entities();
    let pgrx_sql = PgrxSql::build(entities.iter().cloned(), "perf_bench".to_string(), false)
        .expect("graph builds");
    c.bench_function("pgrx_sql_to_sql_64_sql_entities", |b| {
        b.iter(|| black_box(pgrx_sql.to_sql().unwrap()));
    });
}

fn config() -> Criterion {
    Criterion::default()
        .sample_size(50)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
}

criterion_group!(
    name = benches;
    config = config();
    targets = bench_control_file_parse, bench_pgrx_sql_build, bench_pgrx_sql_to_sql
);
criterion_main!(benches);
