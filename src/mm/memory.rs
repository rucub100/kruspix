use core::iter;

pub fn calc_available_mem(
    mut mem: [(usize, usize); 32],
    reserved_mem: &[(usize, usize); 32],
    kernel_region: (usize, usize),
) -> [(usize, usize); 32] {
    let mut mem_skip = 0;
    let mut mem_index = mem.iter().position(|(_, size)| *size == 0).unwrap_or(32);

    loop {
        let mut new_mem = [(0, 0); 32];
        let mut new_mem_index = 0;

        for (mem_addr, mem_size) in mem.iter_mut().filter(|(_, size)| *size != 0).skip(mem_skip) {
            for (rsv_addr, rsv_size) in reserved_mem
                .iter()
                .filter(|(_, size)| *size != 0)
                .chain(iter::once(&kernel_region))
            {
                let diff = region_diff((*mem_addr, *mem_size), (*rsv_addr, *rsv_size));

                *mem_addr = diff[0].0;
                *mem_size = diff[0].1;

                if diff[1].1 != 0 {
                    new_mem[new_mem_index] = diff[1];
                    new_mem_index += 1;
                }
            }
        }

        if new_mem_index == 0 || mem_index >= 32 {
            break;
        }

        mem[mem_index..32].copy_from_slice(&new_mem[0..32 - mem_index]);
        mem_skip = mem_index;
        mem_index += new_mem_index;
    }

    mem.as_mut_slice().sort_unstable_by(|a, b| b.1.cmp(&a.1));

    mem
}

fn region_diff(
    (mem_addr, mem_size): (usize, usize),
    (rsv_addr, rsv_size): (usize, usize),
) -> [(usize, usize); 2] {
    match (mem_addr, mem_size, rsv_addr, rsv_size) {
        (mem_addr, mem_size, rsv_addr, rsv_size)
            if rsv_addr + rsv_size <= mem_addr || mem_addr + mem_size <= rsv_addr =>
        {
            [(mem_addr, mem_size), (0, 0)]
        }
        (mem_addr, mem_size, rsv_addr, rsv_size)
            if rsv_addr <= mem_addr && rsv_addr + rsv_size < mem_addr + mem_size =>
        {
            [
                (
                    rsv_addr + rsv_size,
                    mem_size - (rsv_addr + rsv_size - mem_addr),
                ),
                (0, 0),
            ]
        }
        (mem_addr, _mem_size, rsv_addr, _rsv_size) if rsv_addr <= mem_addr => [(0, 0), (0, 0)],
        (mem_addr, mem_size, rsv_addr, rsv_size) if mem_addr + mem_size <= rsv_addr + rsv_size => {
            [(mem_addr, rsv_addr - mem_addr), (0, 0)]
        }
        _ => [
            (mem_addr, rsv_addr - mem_addr),
            (
                rsv_addr + rsv_size,
                (mem_addr + mem_size) - (rsv_addr + rsv_size),
            ),
        ],
    }
}
