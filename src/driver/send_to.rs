use crate::buf::IoBuf;
use crate::driver::{Op, SharedFd};
use crate::BufResult;
use os_socketaddr::OsSocketAddr;
use std::io::IoSlice;
use std::task::{Context, Poll};
use std::{boxed::Box, io, net::SocketAddr};

pub(crate) struct SendTo<T> {
    #[allow(dead_code)]
    fd: SharedFd,
    pub(crate) buf: T,
    #[allow(dead_code)]
    io_slices: Vec<IoSlice<'static>>,
    #[allow(dead_code)]
    os_socket_addr: Box<OsSocketAddr>,
    pub(crate) msghdr: Box<libc::msghdr>,
}

impl<T: IoBuf> Op<SendTo<T>> {
    pub(crate) fn send_to(
        fd: &SharedFd,
        buf: T,
        socket_addr: SocketAddr,
    ) -> io::Result<Op<SendTo<T>>> {
        use io_uring::{opcode, types};

        let io_slices = vec![IoSlice::new(unsafe {
            std::slice::from_raw_parts(buf.stable_ptr(), buf.bytes_init())
        })];

        let mut os_socket_addr = Box::new(OsSocketAddr::from(socket_addr));

        let mut msghdr: Box<libc::msghdr> = Box::new(unsafe { std::mem::zeroed() });
        msghdr.msg_iov = io_slices.as_ptr() as *mut _;
        msghdr.msg_iovlen = io_slices.len() as _;
        msghdr.msg_name = os_socket_addr.as_mut_ptr() as *mut libc::c_void;
        msghdr.msg_namelen = os_socket_addr.len();

        Op::submit_with(
            SendTo {
                fd: fd.clone(),
                buf,
                io_slices,
                os_socket_addr,
                msghdr,
            },
            |send_to| {
                opcode::SendMsg::new(
                    types::Fd(send_to.fd.raw_fd()),
                    send_to.msghdr.as_ref() as *const _,
                )
                .build()
            },
        )
    }

    pub(crate) async fn send(mut self) -> BufResult<usize, T> {
        use crate::future::poll_fn;

        poll_fn(move |cx| self.poll_send(cx)).await
    }

    pub(crate) fn poll_send(&mut self, cx: &mut Context<'_>) -> Poll<BufResult<usize, T>> {
        use std::future::Future;
        use std::pin::Pin;

        let complete = ready!(Pin::new(self).poll(cx));
        Poll::Ready((complete.result.map(|v| v as _), complete.data.buf))
    }
}
