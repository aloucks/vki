use std::time::Duration;

use vki::FenceError;

pub mod support;

#[test]
fn is_not_signaled() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let queue = device.get_queue();

        let fence = queue.create_fence()?;
        assert_eq!(false, fence.is_signaled());

        Ok(instance)
    });
}

#[test]
fn is_signaled_after_wait() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let queue = device.get_queue();

        let fence1 = queue.create_fence()?;
        assert_eq!(false, fence1.is_signaled());

        let encoder = device.create_command_encoder()?;

        queue.submit(&[encoder.finish()?])?;

        let fence2 = queue.create_fence()?;

        fence1.wait(Duration::from_millis(1_000_000_000))?;
        assert_eq!(true, fence1.is_signaled());

        fence2.wait(Duration::from_millis(1_000_000_000))?;
        assert_eq!(true, fence2.is_signaled());

        Ok(instance)
    });
}

#[test]
fn is_not_signaled_after_reset() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let queue = device.get_queue();
        let encoder = device.create_command_encoder()?;
        queue.submit(&[encoder.finish()?])?;

        let fence = queue.create_fence()?;
        assert_eq!(false, fence.is_signaled(), "fence should not be siganled after create");

        fence.wait(Duration::from_millis(1_000_000_000))?;
        assert_eq!(true, fence.is_signaled(), "fence should be signaled after wait");

        let encoder = device.create_command_encoder()?;
        queue.submit(&[encoder.finish()?])?;

        fence.reset()?;
        assert_eq!(false, fence.is_signaled(), "fence should not be signaled after reset");

        fence.wait(Duration::from_millis(1_000_000_000))?;
        assert_eq!(true, fence.is_signaled(), "fence should be signaled after wait");

        Ok(instance)
    });
}

#[test]
fn timeout() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let queue = device.get_queue();

        let fence = queue.create_fence()?;
        assert_eq!(false, fence.is_signaled());

        queue.submit(&[])?;

        // No commands were submitted. The fence would wait forever..

        let result = fence.wait(Duration::from_millis(100));

        assert_eq!(Err(FenceError::Timeout), result);

        assert_eq!(false, fence.is_signaled());

        Ok(instance)
    });
}
