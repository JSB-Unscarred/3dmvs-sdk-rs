use mv3d_lp::Sdk;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<Sdk>();
}

