use ml_vm::{Compiler, VM};
use ml_core::MLExpr;

fn bench_add(b: &mut criterion::Bencher) {
    let ast = MLExpr::parse("(+ 1 2)").expect("parse failed");
    let module = Compiler::compile(&ast).expect("compile failed");
    let code = module.code;
    let constants = module.constants;
    b.iter(|| {
        let mut vm = VM::new(code.clone(), constants.clone());
        vm.run()
    });
}

criterion::benchmark_group!(vm_benches, bench_add);
criterion::benchmark_main!(vm_benches);
