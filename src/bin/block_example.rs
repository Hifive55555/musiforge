use musiforge::{
    block::{IOData, Time}, create_stream, graph_flow::GraphFlowBuilder
};

fn osc(time: Time, _inputs: &IOData, outputs: &mut IOData) {
    let freq = 440.0;
    let mut time = time;
    for frame in outputs[0].chunks_mut(2) {
        let val = time.get_phase_by_freq(freq).sin();
        // println!("val: {}", val);
        for sample in frame.iter_mut() {
            *sample = val;
        }
        time.tick();
    }
}

fn filter(_time: Time, inputs: &IOData, outputs: &mut IOData) {
    for i in 0..inputs.buffer_size() {
        outputs[0][i] = inputs[0][i];
        // println!("filter: {}", outputs[0][i]);
    }
}

fn main() {
    let builder = GraphFlowBuilder {
        sample_rate: 48000,
        buffer_size: 512,
    };
    let mut gf = builder.build();

    let osc_id = gf.add_block(osc);
    let filter_id = gf.add_block(filter);

    gf.connect(osc_id.with_port(0), filter_id.with_port(0));
    gf.to_output(filter_id.with_port(0));

    println!("{:?}", gf);

    let stream = create_stream(512, 48000, move |data: &mut [f32], _| {
        gf.run(data.len() as u32, data);
    });
    stream();
    std::thread::park();
}