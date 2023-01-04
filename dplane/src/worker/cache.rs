use std::ffi::c_void;
use std::mem::transmute;
use std::sync::Arc;
use std::sync::RwLock;
use crate::core::logger::log::log;
use crate::core::logger::log::debug_log;
use crate::core::memory::ring::Ring;
use crate::core::memory::ring::RingBuf;
use crate::core::memory::ring::init_ringbuf_element;
use crate::core::memory::array::Array;
use crate::cache::cache::CacheElement;
use crate::cache::cache::CacheData;
use crate::cache::tss::TupleSpace;
use crate::cache::tss::KeyStore;
use crate::worker::rx::RxResult;
// use crate::worker::rx::HashCalcResult;


#[repr(C)]
pub struct CacheArgs<'a> {
    pub id: usize,
    pub ring: Ring,

    pub batch_count: usize,
    pub buf_size: usize,
    pub header_max_size: usize,

    // cache
    pub l2_cache: Array<RwLock<CacheElement>>,
    pub l3_cache: &'a TupleSpace<'a>,
    // pub l3_cache: Arc<TupleSpace<'a>>,

    // ring list
    pub pipeline_ring_list: Array<Ring>,
}


// pub struct CacheResult {
//     pub owner_ring: *mut RingBuf<CacheResult>,
//     pub id: usize,
//     pub rx_result: *mut RxResult,
//     pub cache_data: CacheData,
//     pub is_cache_hit: bool,
// }

// impl CacheResult {
//     pub fn free(&mut self) {
//         unsafe {
//             (*self.owner_ring).free(self);
//         }
//     }
// }


fn next_core(mut current_core: usize, core_limit: usize) -> usize {
    current_core += 1;
    if current_core == core_limit {
        current_core = 0;
    }
    current_core
}


pub extern "C" fn start_cache(cache_args_ptr: *mut c_void) -> i32 {
    let cache_args = unsafe { &mut *transmute::<*mut c_void, *mut CacheArgs>(cache_args_ptr) };
    log!("Init Cache{} Core", cache_args.id);

    // init ringbuf
    // let mut cache_result_ring_buf = RingBuf::<CacheResult>::new(cache_args.buf_size);
    // init_ringbuf_element!(cache_result_ring_buf, CacheResult, {
    //     id => cache_args.id,
    //     owner_ring => &mut cache_result_ring_buf as *mut RingBuf<CacheResult>,
    // });


    let rx_result_list = Array::<&mut RxResult>::new(cache_args.batch_count);
    let mut next_pipeline_core = 0;
    let mut tss_key_store = KeyStore {
        key: Array::new(cache_args.header_max_size),
        key_len: 0,
    };

    log!("Start Cache{} Core", cache_args.id);
    loop {
        let rx_result_dequeue_count = cache_args.ring.dequeue_burst::<RxResult>(&rx_result_list, cache_args.batch_count);
        for i in 0..rx_result_dequeue_count {
            debug_log!("Cache{} start", cache_args.id);
            // debug_log!("Cache{} rx_result malloc", cache_args.id);
            // let cache_result = cache_result_ring_buf.malloc();
            // debug_log!("Cache{} done rx_result malloc", cache_args.id);

            let rx_result = rx_result_list.get(i);
            debug_log!("Cache{} start1", cache_args.id);
            let hash_calc_result = unsafe { &mut *rx_result.hash_calc_result };
            debug_log!("Cache{} start2", cache_args.id);

            rx_result.cache_id = cache_args.id;
            rx_result.is_cache_hit = false;
            // cache_result.is_cache_hit = false;
            // cache_result.rx_result = (*rx_result) as *mut RxResult;

            if hash_calc_result.is_lbf_hit {
                debug_log!("Cache{} Check L1 Cache", cache_args.id);
                // l2 cache
                let cache_element = cache_args.l2_cache[hash_calc_result.l2_hash as usize].read().unwrap();
                if cache_element.cmp_ptr_key(hash_calc_result.l2_key.as_ptr(), hash_calc_result.l2_key_len as isize) {
                    debug_log!("Cache{} Hit L2 Cache", cache_args.id);
                    rx_result.cache_data = cache_element.data.clone();
                    rx_result.is_cache_hit = true;

                    debug_log!("Cache{} enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);
                    if cache_args.pipeline_ring_list[next_pipeline_core].enqueue(*rx_result) < 0 {
                        debug_log!("Cache{} failed enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);
                        rx_result.pktbuf.free();
                        unsafe { (*rx_result.hash_calc_result).free(); };
                        rx_result.free();
                        // cache_result.free();
                        continue;
                    }
                    debug_log!("Cache{} complete enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);
                    next_pipeline_core = next_core(next_pipeline_core, cache_args.pipeline_ring_list.len());
                    continue;
                }
                debug_log!("Cache{} No Hit L2 Cache", cache_args.id);
            }


            // l3 cache (tss)
            // let cache_data = cache_args.l3_cache.search(rx_result.raw_pkt, &mut tss_key_store);
            // match cache_data {
            //     Some(cache_data) => {
            //         cache_result.cache_data = cache_data;
            //         cache_result.is_cache_hit = true;
            //     },
            //     None => {},
            // }

            debug_log!("Cache{} enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);
            if cache_args.pipeline_ring_list[next_pipeline_core].enqueue(*rx_result) < 0 {
                debug_log!("Cache{} failed enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);
                rx_result.pktbuf.free();
                unsafe { (*rx_result.hash_calc_result).free(); };
                rx_result.free();
                // cache_result.free();
                continue;
            }
            debug_log!("Cache{} complete enqueue to Pipeline Core {}", cache_args.id, next_pipeline_core);

            next_pipeline_core = next_core(next_pipeline_core, cache_args.pipeline_ring_list.len());
            debug_log!("Cache{} continue", cache_args.id);
            continue;
        }


        if false {
            return 0;
        }
    }
    // 0
}
