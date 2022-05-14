use std::mem::ManuallyDrop;
use std::ops::DerefMut;
use std::pin::Pin;

pub fn pin_manually_drop_as_mut<P, T>(pin: &mut Pin<P>) -> Pin<&mut T>
where
    P: DerefMut<Target = ManuallyDrop<T>>,
{
    unsafe { Pin::new_unchecked(&mut *pin.as_mut().get_unchecked_mut()) }
}
