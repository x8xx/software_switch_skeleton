use std::sync::RwLock;
use crate::core::memory::array::Array;
use crate::core::memory::ring::Ring;
use crate::cache::cache::CacheElement;
// use crate::cache::cache::CacheData;
use crate::pipeline::table::Table;
use crate::worker::pipeline::NewCacheElement;


pub fn create_new_cache(ring: Ring, 
                        table_list: Array<RwLock<Table>>,
                        l1_cache_list: Array<Array<RwLock<CacheElement>>>,
                        lbf_list: Array<Array<u64>>,
                        l2_cache_list: Array<Array<Array<RwLock<CacheElement>>>>) {
    let new_cache_list = Array::<&mut NewCacheElement>::new(32);
    loop {
        let new_cache_dequeue_count = ring.dequeue_burst::<NewCacheElement>(&new_cache_list, 32);
        for i in 0..new_cache_dequeue_count {
            let new_cache = new_cache_list.get(i);
            let hash_calc_result = unsafe { &mut *new_cache.hash_calc_result };

            // L1 Cache
            {
                let mut l1_cache = l1_cache_list[new_cache.rx_id][hash_calc_result.l1_hash as usize].write().unwrap();
                l1_cache.key_len = new_cache.l1_key_len as isize;
                new_cache.l1_key.deepcopy(&mut l1_cache.key);
                new_cache.cache_data.deepcopy(&mut l1_cache.data);
            }

            // L2 Cache
            {
                let mut l2_cache = l2_cache_list[new_cache.rx_id][new_cache.cache_id][hash_calc_result.l2_hash as usize].write().unwrap();
                l2_cache.key_len = hash_calc_result.l2_key_len as isize;
                hash_calc_result.l2_key.deepcopy(&mut l2_cache.key);
                new_cache.cache_data.deepcopy(&mut l2_cache.data);
            }

            hash_calc_result.free();
            new_cache.free();
        }
    }
}
