--- test_mov32_imm
add64 r10, 0
mov32 r0, 1
exit
--- test_mov32_imm
add64 r10, 0
mov32 r0, -1
exit
--- test_mov32_reg
add64 r10, 0
mov32 r1, 1
mov32 r0, r1
exit
--- test_mov32_reg
add64 r10, 0
mov32 r1, -1
mov32 r0, r1
exit
--- test_mov64_imm
add64 r10, 0
mov64 r0, 1
exit
--- test_mov64_imm
add64 r10, 0
mov64 r0, -1
exit
--- test_mov64_reg
add64 r10, 0
mov64 r1, 1
mov64 r0, r1
exit
--- test_mov64_reg
add64 r10, 0
mov64 r1, -1
mov64 r0, r1
exit
--- test_bounce
add64 r10, 0
mov r0, 1
mov r6, r0
mov r7, r6
mov r8, r7
mov r9, r8
mov r0, r9
exit
--- test_add32_sub32
add64 r10, 0
mov32 r0, 1
add32 r0, 2
mov32 r1, 5
add32 r0, r1
mov32 r1, 3
sub32 r0, r1
exit
--- test_add64_sub64
add64 r10, 0
mov32 r0, 1
add64 r0, 2
mov32 r1, 5
add64 r0, r1
mov32 r1, 3
sub64 r0, r1
exit
--- test_lmul128
add64 r10, 0
mov r0, r1
mov r2, 30
mov r3, 0
mov r4, 20
mov r5, 0
mul64 r3, r4
mul64 r5, r2
add64 r5, r3
mov64 r0, r2
rsh64 r0, 0x20
mov64 r3, r4
rsh64 r3, 0x20
mov64 r6, r3
mul64 r6, r0
add64 r5, r6
lsh64 r4, 0x20
rsh64 r4, 0x20
mov64 r6, r4
mul64 r6, r0
lsh64 r2, 0x20
rsh64 r2, 0x20
mul64 r4, r2
mov64 r0, r4
rsh64 r0, 0x20
add64 r0, r6
mov64 r6, r0
rsh64 r6, 0x20
add64 r5, r6
mul64 r3, r2
lsh64 r0, 0x20
rsh64 r0, 0x20
add64 r0, r3
mov64 r2, r0
rsh64 r2, 0x20
add64 r5, r2
stxdw [r1+0x8], r5
lsh64 r0, 0x20
lsh64 r4, 0x20
rsh64 r4, 0x20
or64 r0, r4
stxdw [r1+0x0], r0
exit
--- test_alu32_logic
add64 r10, 0
mov32 r0, 0
mov32 r1, 1
mov32 r2, 2
mov32 r3, 3
mov32 r4, 4
mov32 r5, 5
mov32 r6, 6
mov32 r7, 7
mov32 r8, 8
or32 r0, r5
or32 r0, 0xa0
and32 r0, 0xa3
mov32 r9, 0x91
and32 r0, r9
lsh32 r0, 22
lsh32 r0, r8
rsh32 r0, 19
rsh32 r0, r7
xor32 r0, 0x03
xor32 r0, r2
exit
--- test_alu64_logic
add64 r10, 0
mov r0, 0
mov r1, 1
mov r2, 2
mov r3, 3
mov r4, 4
mov r5, 5
mov r6, 6
mov r7, 7
mov r8, 8
or r0, r5
or r0, 0xa0
and r0, 0xa3
mov r9, 0x91
and r0, r9
lsh r0, 32
lsh r0, 22
lsh r0, r8
rsh r0, 32
rsh r0, 19
rsh r0, r7
xor r0, 0x03
xor r0, r2
exit
--- test_arsh32_high_shift
add64 r10, 0
mov r0, 8
lddw r1, 0x100000001
arsh32 r0, r1
exit
--- test_arsh32_imm
add64 r10, 0
mov32 r0, 0xf8
lsh32 r0, 28
arsh32 r0, 16
exit
--- test_arsh32_reg
add64 r10, 0
mov32 r0, 0xf8
mov32 r1, 16
lsh32 r0, 28
arsh32 r0, r1
exit
--- test_arsh64
add64 r10, 0
mov32 r0, 1
lsh r0, 63
arsh r0, 55
mov32 r1, 5
arsh r0, r1
exit
--- test_lsh64_reg
add64 r10, 0
mov r0, 0x1
mov r7, 4
lsh r0, r7
exit
--- test_rhs32_imm
add64 r10, 0
xor r0, r0
add r0, -1
rsh32 r0, 8
exit
--- test_rsh64_reg
add64 r10, 0
mov r0, 0x10
mov r7, 4
rsh r0, r7
exit
--- test_le16
add64 r10, 0
lddw r0, 0x2211
le16 r0
exit
--- test_le16_high
add64 r10, 0
lddw r0, 0x8877665544332211
le16 r0
exit
--- test_le32
add64 r10, 0
lddw r0, 0x44332211
le32 r0
exit
--- test_le32_high
add64 r10, 0
lddw r0, 0x8877665544332211
le32 r0
exit
--- test_le64
add64 r10, 0
lddw r0, 0x8877665544332211
le64 r0
exit
--- test_be16
add64 r10, 0
lddw r0, 0x2211
be16 r0
exit
--- test_be16_high
add64 r10, 0
lddw r0, 0x8877665544332211
be16 r0
exit
--- test_be32
add64 r10, 0
lddw r0, 0x44332211
be32 r0
exit
--- test_be32_high
add64 r10, 0
lddw r0, 0x8877665544332211
be32 r0
exit
--- test_be64
add64 r10, 0
lddw r0, 0x8877665544332211
be64 r0
exit
--- test_memory_instructions
add64 r10, 0
ldxb r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
ldxh r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
ldxw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stb [r1+2], 0x11
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stb [r1+2], -1
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
sth [r1+2], 0x2211
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
sth [r1+2], -1
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stw [r1+2], 0x44332211
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stw [r1+2], -1
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stdw [r1+2], 0x44332211
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
stdw [r1+2], -1
ldxdw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
mov32 r2, 0x11
stxb [r1+2], r2
ldxb r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
mov32 r2, 0x2211
stxh [r1+2], r2
ldxh r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
mov32 r2, 0x44332211
stxw [r1+2], r2
ldxw r0, [r1+2]
exit
--- test_memory_instructions
add64 r10, 0
mov r2, -2005440939
lsh r2, 32
or r2, 0x44332211
stxdw [r1+2], r2
ldxdw r0, [r1+2]
exit
--- test_hor64
add64 r10, 0
hor64 r0, 0x10203040
hor64 r0, 0x01020304
exit
--- test_hor64
add64 r10, 0
mov32 r0, 0x12345678
hor64 r0, -1
exit
--- test_ldxh_same_reg
add64 r10, 0
mov r0, r1
sth [r0], 0x1234
ldxh r0, [r0]
exit
--- test_err_ldxdw_oob
add64 r10, 0
ldxdw r0, [r1+6]
exit
--- test_err_ldxdw_nomem
add64 r10, 0
ldxdw r0, [r1+6]
exit
--- test_err_ldxdw_nomem_capped
add64 r10, 0
ldxdw r0, [r1+6]
exit
--- test_memory_access_preserves_registers
mov64 r0, 0x1
mov64 r1, 0x2
mov64 r2, 0x4
mov64 r3, 0x8
mov64 r4, 0x10
mov64 r5, 0x20
mov64 r6, 0x40
mov64 r7, 0x80
mov64 r8, 0x100
mov64 r9, 0x200
stxdw [r10-8], r9
stdw [r10-16], 0x400
ldxdw r9, [r10-8]
ldxdw r9, [r10-16]
add64 r0, r1
add64 r0, r2
add64 r0, r3
add64 r0, r4
add64 r0, r5
add64 r0, r6
add64 r0, r7
add64 r0, r8
add64 r0, r9
exit
--- test_ldxb_all
add64 r10, 0
mov r0, r1
ldxb r9, [r0+0]
lsh r9, 0
ldxb r8, [r0+1]
lsh r8, 4
ldxb r7, [r0+2]
lsh r7, 8
ldxb r6, [r0+3]
lsh r6, 12
ldxb r5, [r0+4]
lsh r5, 16
ldxb r4, [r0+5]
lsh r4, 20
ldxb r3, [r0+6]
lsh r3, 24
ldxb r2, [r0+7]
lsh r2, 28
ldxb r1, [r0+8]
lsh r1, 32
ldxb r0, [r0+9]
lsh r0, 36
or r0, r1
or r0, r2
or r0, r3
or r0, r4
or r0, r5
or r0, r6
or r0, r7
or r0, r8
or r0, r9
exit
--- test_ldxh_all
add64 r10, 0
mov r0, r1
ldxh r9, [r0+0]
be16 r9
lsh r9, 0
ldxh r8, [r0+2]
be16 r8
lsh r8, 4
ldxh r7, [r0+4]
be16 r7
lsh r7, 8
ldxh r6, [r0+6]
be16 r6
lsh r6, 12
ldxh r5, [r0+8]
be16 r5
lsh r5, 16
ldxh r4, [r0+10]
be16 r4
lsh r4, 20
ldxh r3, [r0+12]
be16 r3
lsh r3, 24
ldxh r2, [r0+14]
be16 r2
lsh r2, 28
ldxh r1, [r0+16]
be16 r1
lsh r1, 32
ldxh r0, [r0+18]
be16 r0
lsh r0, 36
or r0, r1
or r0, r2
or r0, r3
or r0, r4
or r0, r5
or r0, r6
or r0, r7
or r0, r8
or r0, r9
exit
--- test_ldxh_all2
add64 r10, 0
mov r0, r1
ldxh r9, [r0+0]
be16 r9
ldxh r8, [r0+2]
be16 r8
ldxh r7, [r0+4]
be16 r7
ldxh r6, [r0+6]
be16 r6
ldxh r5, [r0+8]
be16 r5
ldxh r4, [r0+10]
be16 r4
ldxh r3, [r0+12]
be16 r3
ldxh r2, [r0+14]
be16 r2
ldxh r1, [r0+16]
be16 r1
ldxh r0, [r0+18]
be16 r0
or r0, r1
or r0, r2
or r0, r3
or r0, r4
or r0, r5
or r0, r6
or r0, r7
or r0, r8
or r0, r9
exit
--- test_ldxw_all
add64 r10, 0
mov r0, r1
ldxw r9, [r0+0]
be32 r9
ldxw r8, [r0+4]
be32 r8
ldxw r7, [r0+8]
be32 r7
ldxw r6, [r0+12]
be32 r6
ldxw r5, [r0+16]
be32 r5
ldxw r4, [r0+20]
be32 r4
ldxw r3, [r0+24]
be32 r3
ldxw r2, [r0+28]
be32 r2
ldxw r1, [r0+32]
be32 r1
ldxw r0, [r0+36]
be32 r0
or r0, r1
or r0, r2
or r0, r3
or r0, r4
or r0, r5
or r0, r6
or r0, r7
or r0, r8
or r0, r9
exit
--- test_stxb_all
add64 r10, 0
mov r0, 0xf0
mov r2, 0xf2
mov r3, 0xf3
mov r4, 0xf4
mov r5, 0xf5
mov r6, 0xf6
mov r7, 0xf7
mov r8, 0xf8
stxb [r1], r0
stxb [r1+1], r2
stxb [r1+2], r3
stxb [r1+3], r4
stxb [r1+4], r5
stxb [r1+5], r6
stxb [r1+6], r7
stxb [r1+7], r8
ldxdw r0, [r1]
be64 r0
exit
--- test_stxb_all2
add64 r10, 0
mov r0, r1
mov r1, 0xf1
mov r9, 0xf9
stxb [r0], r1
stxb [r0+1], r9
ldxh r0, [r0]
be16 r0
exit
--- test_stxb_chain
add64 r10, 0
mov r0, r1
ldxb r9, [r0+0]
stxb [r0+1], r9
ldxb r8, [r0+1]
stxb [r0+2], r8
ldxb r7, [r0+2]
stxb [r0+3], r7
ldxb r6, [r0+3]
stxb [r0+4], r6
ldxb r5, [r0+4]
stxb [r0+5], r5
ldxb r4, [r0+5]
stxb [r0+6], r4
ldxb r3, [r0+6]
stxb [r0+7], r3
ldxb r2, [r0+7]
stxb [r0+8], r2
ldxb r1, [r0+8]
stxb [r0+9], r1
ldxb r0, [r0+9]
exit
--- test_exit_capped
add64 r10, 0
exit
--- test_exit_without_value
add64 r10, 0
exit
--- test_exit
add64 r10, 0
mov r0, 0
exit
--- test_early_exit
add64 r10, 0
mov r0, 3
exit
mov r0, 4
exit
--- test_ja
add64 r10, 0
mov r0, 1
ja +1
mov r0, 2
exit
--- test_alu64_imm_sign_extension
mov64 r0, 0
or64 r0, -1
mov64 r1, -1
and64 r1, -2
mov64 r2, 0
xor64 r2, -4
add64 r0, r1
add64 r0, r2
exit
--- test_instruction_meter_checkpoint
mov64 r0, 0
stb [r1+0], 1
stb [r1+1], 2
stb [r1+2], 3
stb [r1+3], 4
exit
--- test_noop_insertion
mov64 r0, 0
mov64 r1, 0
lddw r2, 0x100000000
add64 r1, 1
call function_foo
jlt r1, 10, -3
exit
function_foo:
add64 r0, r2
exit
--- test_jset_upper_half
lddw r1, 0x100000000
mov64 r0, 0
jset r1, -1, +1
mov64 r0, 1
exit
--- test_jset_upper_half
lddw r1, 0x100000000
mov64 r2, -1
mov64 r0, 0
jset r1, r2, +1
mov64 r0, 1
exit
--- test_stack1
add64 r10, 64
mov r1, 51
stdw [r10-16], 0xab
stdw [r10-8], 0xcd
and r1, 1
lsh r1, 3
mov r2, r10
add r2, r1
ldxdw r0, [r2-16]
exit
--- test_stack2
add64 r10, 64
stb [r10-4], 0x01
stb [r10-3], 0x02
stb [r10-2], 0x03
stb [r10-1], 0x04
mov r1, r10
mov r2, 0x4
sub r1, r2
syscall bpf_mem_frob
mov r1, 0
ldxb r2, [r10-4]
ldxb r3, [r10-3]
ldxb r4, [r10-2]
ldxb r5, [r10-1]
syscall bpf_gather_bytes
xor r0, 0x2a2a2a2a
exit
--- test_string_stack
add64 r10, 64
mov r1, 0x78636261
stxw [r10-8], r1
mov r6, 0x0
stxb [r10-4], r6
stxb [r10-12], r6
mov r1, 0x79636261
stxw [r10-16], r1
mov r1, r10
add r1, -8
mov r2, r1
syscall bpf_str_cmp
mov r1, r0
mov r0, 0x1
lsh r1, 0x20
rsh r1, 0x20
jne r1, 0x0, +11
mov r1, r10
add r1, -8
mov r2, r10
add r2, -16
syscall bpf_str_cmp
mov r1, r0
lsh r1, 0x20
rsh r1, 0x20
mov r0, 0x1
jeq r1, r6, +1
mov r0, 0x0
exit
--- test_err_stack_out_of_bound
stb [r10-0x1001], 0
exit
--- test_err_stack_out_of_bound
mov r0, r10
mov r1, 63
lsh r1, 12
add r0, r1
stb [r0], 0
exit
--- test_dynamic_stack_frames
add64 r10, 0
call function_foo
exit
function_foo:
add64 r10, 0
mov r0, r10
exit
--- test_dynamic_stack_frames
add r10, -0x7FFFFF00
call function_stage1
exit
function_stage1:
add r10, -0x7FFFFF00
call function_stage2
exit
function_stage2:
add r10, -0x7FFFFF00
call function_stage3
exit
function_stage3:
add r10, -0x7FFFFF00
call function_stage4
exit
function_stage4:
add r10, -0x1440
call function_final
exit
function_final:
add r10, 0
stb [r10], 0
exit
--- test_dynamic_stack_frames
add r10, -64
stxdw [r10-8], r10
call function_foo
ldxdw r0, [r10-8]
exit
function_foo:
add r10, 0
exit
--- test_dynamic_stack_frames
add r10, -64
call function_foo
exit
function_foo:
add r10, 0
mov r0, r10
exit
--- test_dynamic_stack_frames
add64 r10, 0
call function_foo
mov r0, r10
exit
function_foo:
add r10, -64
exit
--- test_entrypoint_exit
entrypoint:
add64 r10, 0
call function_foo
mov r0, 42
exit
function_foo:
add64 r10, 0
mov r0, 12
exit
--- test_stack_call_depth_tracking
add64 r10, 0
call function_foo
call function_foo
exit
function_foo:
add64 r10, 0
exit
--- test_stack_call_depth_tracking
entrypoint:
add64 r10, 0
call function_foo
exit
function_foo:
add64 r10, 0
call function_bar
exit
function_bar:
add64 r10, 0
exit
--- test_bpf_to_bpf_scratch_registers
add64 r10, 0
mov64 r6, 0x11
mov64 r7, 0x22
mov64 r8, 0x44
mov64 r9, 0x88
call function_foo
mov64 r0, r6
add64 r0, r7
add64 r0, r8
add64 r0, r9
exit
function_foo:
add64 r10, 0
mov64 r6, 0x00
mov64 r7, 0x00
mov64 r8, 0x00
mov64 r9, 0x00
exit
--- test_syscall_parameter_on_stack
add64 r10, 0x100
mov64 r1, r10
add64 r1, -0x100
mov64 r2, 0x1
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_callx
add64 r10, 0
mov64 r0, 0x0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x38
callx r8
exit
function_foo:
add64 r10, 0
mov64 r0, 0x2A
exit
--- test_callx_src_reg
add64 r10, 0
mov64 r0, 0x0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x38
callx r8
exit
function_foo:
mov64 r0, 0x2A
exit
--- test_callx_unaligned_target
mov64 r0, 0x0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x33
callx r8
exit
function_foo:
mov64 r0, 0x2A
exit
--- test_err_callx_oob_computed
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x28
callx r8
exit
--- test_err_callx_oob_computed
mov64 r8, 0x1
lsh64 r8, 0x20
sub64 r8, 0x1
callx r8
exit
--- test_err_callx_oob_low
add64 r10, 0
mov64 r0, 0x3
callx r0
exit
--- test_err_callx_oob_high
add64 r10, 0
lddw r0, 0x200000000
callx r0
exit
--- test_err_callx_oob_max
add64 r10, 0
lddw r0, 0xFFFFFFFFFFFFFFF8
callx r0
exit
--- test_bpf_to_bpf_depth
add64 r10, 0
ldxb r1, [r1]
add64 r1, -2
call function_foo
exit
function_foo:
add64 r10, 0
jeq r1, 0, +2
add64 r1, -1
call function_foo
exit
--- test_bpf_to_bpf_depth
add64 r10, 0
ldxb r1, [r1]
add64 r1, -2
call function_foo
exit
function_foo:
add64 r10, 0
jeq r1, 0, +2
add64 r1, -1
call function_foo
exit
--- test_err_reg_stack_depth
add64 r10, 0
mov64 r0, 0x1
lsh64 r0, 0x20
callx r0
exit
--- test_err_reg_stack_depth
add64 r10, 0
mov64 r0, 0x1
lsh64 r0, 0x20
callx r0
exit
--- test_call_save
mov64 r6, 0x1
mov64 r7, 0x20
mov64 r8, 0x300
mov64 r9, 0x4000
call 0
mov64 r0, 0x0
or64 r0, r6
or64 r0, r7
or64 r0, r8
or64 r0, r9
exit
--- test_err_syscall_string
add64 r10, 0
mov64 r1, 0x0
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_syscall_string
add64 r10, 0
mov64 r2, 0x5
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_syscall
add64 r10, 0
mov64 r1, 0xAA
mov64 r2, 0xBB
mov64 r3, 0xCC
mov64 r4, 0xDD
mov64 r5, 0xEE
syscall bpf_syscall_u64
mov64 r0, 0x0
exit
--- test_err_syscall_error
mov64 r0, 0x7
syscall bpf_syscall_err
mov64 r0, 0x0
exit
--- test_syscall_preserves_registers
mov64 r1, 0x1
mov64 r2, 0x2
mov64 r3, 0x4
mov64 r4, 0x8
mov64 r5, 0x10
mov64 r6, 0x20
mov64 r7, 0x40
mov64 r8, 0x80
mov64 r9, 0x100
mov64 r0, 0x200
syscall bpf_syscall_clobber
add64 r0, r1
add64 r0, r2
add64 r0, r3
add64 r0, r4
add64 r0, r5
add64 r0, r6
add64 r0, r7
add64 r0, r8
add64 r0, r9
add64 r0, r10
exit
--- test_syscall_in_internal_call
mov r1, 1
call function_foo
add64 r0, 1
exit
function_foo:
mov r2, 2
mov r3, 3
mov r4, 4
mov r5, 5
syscall bpf_gather_bytes
exit
--- test_err_syscall_error_capped
mov64 r0, 0x7
mov64 r0, 0x7
syscall bpf_syscall_err
exit
--- test_call_gather_bytes
add64 r10, 0
mov r1, 1
mov r2, 2
mov r3, 3
mov r4, 4
mov r5, 5
syscall bpf_gather_bytes
exit
--- test_call_memfrob
add64 r10, 0
mov r6, r1
add r1, 2
mov r2, 4
syscall bpf_mem_frob
ldxdw r0, [r6]
be64 r0
exit
--- test_tight_infinite_loop_conditional
add64 r10, 0
jsge r0, r0, -1
exit
--- test_tight_infinite_loop_unconditional
add64 r10, 0
ja -1
exit
--- test_tight_infinite_recursion
entrypoint:
add64 r10, 0
mov64 r3, 0x41414141
call entrypoint
exit
--- test_entrypoint_not_first
function_foo:
mov64 r0, 3
exit
entrypoint:
call function_foo
add64 r0, 4
exit
--- test_nested_calls_return
mov64 r6, 40
loop:
call function_foo
sub64 r6, 1
jne r6, 0, loop
mov64 r0, 7
exit
function_foo:
call function_bar
exit
function_bar:
exit
--- test_tight_infinite_recursion_callx
add64 r10, 0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x30
call function_foo
exit
function_foo:
add64 r10, 0
callx r8
exit
--- test_instruction_count_syscall
add64 r10, 0
mov64 r2, 0x5
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_err_instruction_count_syscall_capped
add64 r10, 0
mov64 r2, 0x5
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_err_non_terminate_capped
add64 r10, 0
mov64 r6, 0x0
mov64 r1, 0x0
mov64 r2, 0x0
mov64 r3, 0x0
mov64 r4, 0x0
mov64 r5, r6
add64 r6, 0x1
ja -0x8
exit
--- test_err_non_terminate_capped
add64 r10, 0
mov64 r6, 0x0
mov64 r1, 0x0
mov64 r2, 0x0
mov64 r3, 0x0
mov64 r4, 0x0
mov64 r5, r6
add64 r6, 0x1
ja -0x8
exit
--- test_err_capped_before_exception
add64 r10, 0
mov64 r1, 0x0
mov64 r2, 0x0
div64 r1, r2
mov64 r0, 0x0
exit
--- test_err_capped_before_exception
add64 r10, 0
mov64 r1, 0x0
mov64 r2, 0x0
callx r2
mov64 r0, 0x0
exit
--- test_err_exit_capped
add64 r10, 0
mov64 r0, 0x1
lsh64 r0, 0x20
or64 r0, 0x30
callx r0
exit
function_foo:
add64 r10, 0
exit
--- test_err_exit_capped
add64 r10, 0
mov64 r0, 0x1
lsh64 r0, 0x20
or64 r0, 0x30
callx r0
exit
function_foo:
add64 r10, 0
mov r0, r0
exit
--- test_err_exit_capped
add64 r10, 0
call function_foo
exit
function_foo:
add64 r10, 0
mov r0, r0
exit
--- test_far_jumps
add64 r10, 0
call function_c
exit
function_a:
add64 r10, 0
exit
function_b:
.fill 1024, 0x0F
exit
function_c:
add64 r10, 0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x18
callx r8
exit
--- test_err_call_unresolved
add64 r10, 0
mov r1, 1
mov r2, 2
mov r3, 3
mov r4, 4
mov r5, 5
syscall Unresolved
mov64 r0, 0x0
exit
--- test_lmul_loop
add64 r10, 0
mov r0, 0x7
add r1, 0xa
lsh r1, 0x20
rsh r1, 0x20
jeq r1, 0x0, +4
mov r0, 0x7
mul r0, 0x7
add r1, -1
jne r1, 0x0, -3
exit
--- test_prime
add64 r10, 0
mov r1, 67
mov r0, 0x1
mov r2, 0x2
jgt r1, 0x2, +4
ja +10
add r2, 0x1
mov r0, 0x1
jge r2, r1, +7
mov r3, r1
div r3, r2
mul r3, r2
mov r4, r1
sub r4, r3
mov r0, 0x0
jne r4, 0x0, -10
exit
--- test_subnet
add64 r10, 0
mov r2, 0xe
ldxh r3, [r1+12]
jne r3, 0x81, +2
mov r2, 0x12
ldxh r3, [r1+16]
and r3, 0xffff
jne r3, 0x8, +5
add r1, r2
mov r0, 0x1
ldxw r1, [r1+16]
and r1, 0xffffff
jeq r1, 0x1a8c0, +1
mov r0, 0x0
exit
--- test_tcp_port80_match
add64 r10, 0
ldxb r2, [r1+0xc]
ldxb r3, [r1+0xd]
lsh64 r3, 0x8
or64 r3, r2
mov64 r0, 0x0
jne r3, 0x8, +0xc
ldxb r2, [r1+0x17]
jne r2, 0x6, +0xa
ldxb r2, [r1+0xe]
add64 r1, 0xe
and64 r2, 0xf
lsh64 r2, 0x2
add64 r1, r2
ldxh r2, [r1+0x2]
jeq r2, 0x5000, +0x2
ldxh r1, [r1+0x0]
jne r1, 0x5000, +0x1
mov64 r0, 0x1
exit
--- test_tcp_port80_nomatch
add64 r10, 0
ldxb r2, [r1+0xc]
ldxb r3, [r1+0xd]
lsh64 r3, 0x8
or64 r3, r2
mov64 r0, 0x0
jne r3, 0x8, +0xc
ldxb r2, [r1+0x17]
jne r2, 0x6, +0xa
ldxb r2, [r1+0xe]
add64 r1, 0xe
and64 r2, 0xf
lsh64 r2, 0x2
add64 r1, r2
ldxh r2, [r1+0x2]
jeq r2, 0x5000, +0x2
ldxh r1, [r1+0x0]
jne r1, 0x5000, +0x1
mov64 r0, 0x1
exit
--- test_tcp_port80_nomatch_ethertype
add64 r10, 0
ldxb r2, [r1+0xc]
ldxb r3, [r1+0xd]
lsh64 r3, 0x8
or64 r3, r2
mov64 r0, 0x0
jne r3, 0x8, +0xc
ldxb r2, [r1+0x17]
jne r2, 0x6, +0xa
ldxb r2, [r1+0xe]
add64 r1, 0xe
and64 r2, 0xf
lsh64 r2, 0x2
add64 r1, r2
ldxh r2, [r1+0x2]
jeq r2, 0x5000, +0x2
ldxh r1, [r1+0x0]
jne r1, 0x5000, +0x1
mov64 r0, 0x1
exit
--- test_tcp_port80_nomatch_proto
add64 r10, 0
ldxb r2, [r1+0xc]
ldxb r3, [r1+0xd]
lsh64 r3, 0x8
or64 r3, r2
mov64 r0, 0x0
jne r3, 0x8, +0xc
ldxb r2, [r1+0x17]
jne r2, 0x6, +0xa
ldxb r2, [r1+0xe]
add64 r1, 0xe
and64 r2, 0xf
lsh64 r2, 0x2
add64 r1, r2
ldxh r2, [r1+0x2]
jeq r2, 0x5000, +0x2
ldxh r1, [r1+0x0]
jne r1, 0x5000, +0x1
mov64 r0, 0x1
exit
--- test_tcp_sack_match
add64 r10, 0
ldxb r2, [r1+12]
ldxb r3, [r1+13]
lsh r3, 0x8
or r3, r2
mov r0, 0x0
jne r3, 0x8, +37
ldxb r2, [r1+23]
jne r2, 0x6, +35
ldxb r2, [r1+14]
add r1, 0xe
and r2, 0xf
lsh r2, 0x2
add r1, r2
mov r0, 0x0
ldxh r4, [r1+12]
add r1, 0x14
rsh r4, 0x2
and r4, 0x3c
mov r2, r4
add r2, -20
mov r5, 0x15
mov r3, 0x0
jgt r5, r4, +20
mov r5, r3
lsh r5, 0x20
arsh r5, 0x20
mov r4, r1
add r4, r5
ldxb r5, [r4]
jeq r5, 0x1, +4
jeq r5, 0x0, +12
mov r6, r3
jeq r5, 0x5, +9
ja +2
add r3, 0x1
mov r6, r3
ldxb r3, [r4+1]
add r3, r6
lsh r3, 0x20
arsh r3, 0x20
jsgt r2, r3, -18
ja +1
mov r0, 0x1
exit
--- test_tcp_sack_nomatch
add64 r10, 0
ldxb r2, [r1+12]
ldxb r3, [r1+13]
lsh r3, 0x8
or r3, r2
mov r0, 0x0
jne r3, 0x8, +37
ldxb r2, [r1+23]
jne r2, 0x6, +35
ldxb r2, [r1+14]
add r1, 0xe
and r2, 0xf
lsh r2, 0x2
add r1, r2
mov r0, 0x0
ldxh r4, [r1+12]
add r1, 0x14
rsh r4, 0x2
and r4, 0x3c
mov r2, r4
add r2, -20
mov r5, 0x15
mov r3, 0x0
jgt r5, r4, +20
mov r5, r3
lsh r5, 0x20
arsh r5, 0x20
mov r4, r1
add r4, r5
ldxb r5, [r4]
jeq r5, 0x1, +4
jeq r5, 0x0, +12
mov r6, r3
jeq r5, 0x5, +9
ja +2
add r3, 0x1
mov r6, r3
ldxb r3, [r4+1]
add r3, r6
lsh r3, 0x20
arsh r3, 0x20
jsgt r2, r3, -18
ja +1
mov r0, 0x1
exit
--- test_call_imm_does_not_dispatch_syscalls
add64 r10, 0
call function_foo
exit
syscall bpf_syscall_string
exit
function_foo:
add64 r10, 0
mov r0, 42
exit
--- test_callx_unsupported_instruction_and_exceeded_max_instructions
add64 r10, 0
sub32 r7, r1
add64 r5, -8
add64 r7, 0
callx r5
exit
--- test_capped_after_callx
add64 r10, 0
mov64 r0, 0x0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x38
callx r8
exit
function_foo:
add64 r10, 0
mov64 r0, 0x2A
exit
--- test_err_fixed_stack_out_of_bound
add64 r10, 0
stb [r10-0x4000], 0
exit
--- test_execution_overrun
add64 r10, 0
add r1, 0
--- test_execution_overrun
add64 r10, 0
add r1, 0
--- test_execution_overrun
add64 r10, 0
add r1, 0
--- test_mov32_reg_truncating
add64 r10, 0
mov64 r1, -1
mov32 r0, r1
exit
--- test_lddw
add64 r10, 0
lddw r0, 0x1122334455667788
--- test_lddw
add64 r10, 0
lddw r0, 0x1122334455667788
exit
--- test_lddw
add64 r10, 0
lddw r0, 0x0000000080000000
exit
--- test_lddw
add64 r10, 0
lddw r0, 0x80000000ffffffff
exit
--- test_lddw
add64 r10, 0
mov r0, 0
mov r1, 0
mov r2, 0
lddw r0, 0x1
ja +2
lddw r1, 0x1
lddw r2, 0x1
add r1, r2
add r0, r1
exit
--- test_lddw
add64 r10, 0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x30
callx r8
lddw r0, 0x1122334455667788
exit
--- test_lddw
add64 r10, 0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x30
callx r8
lddw r0, 0x1122334455667788
exit
--- test_lddw
add64 r10, 0
mov64 r1, 0x1
lsh64 r1, 0x20
or64 r1, 0x40
callx r1
mov r0, r0
mov r0, r0
lddw r0, 0x1122334455667788
exit
--- test_lddw
add64 r10, 0
lddw r1, 0x100000040
callx r1
mov r0, r0
mov r0, r0
exit
lddw r0, 0x1122334455667788
exit
--- test_lddw
add64 r10, 0
mov r0, 0
lddw r1, 0x1
mov r2, 0
exit
--- test_lddw_call_target
call function_foo
exit
function_foo:
lddw r0, 0x100000002
exit
--- test_err_callx_into_lddw
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x28
callx r8
lddw r0, 0x1
exit
--- test_neg
add64 r10, 0
mov32 r0, 2
neg32 r0
exit
--- test_neg
add64 r10, 0
mov r0, 2
neg r0
exit
--- test_neg
add64 r10, 0
mov32 r0, 3
sub32 r0, 1
exit
--- test_neg
add64 r10, 0
mov r0, 3
sub r0, 1
exit
--- test_callx_imm
add64 r10, 0
mov64 r0, 0x0
mov64 r8, 0x1
lsh64 r8, 0x20
or64 r8, 0x38
callx r8
exit
function_foo:
mov64 r0, 0x2A
exit
--- test_mul
add64 r10, 0
mov r0, 3
mul32 r0, 4
exit
--- test_mul
add64 r10, 0
mov r0, 3
mov r1, 4
mul32 r0, r1
exit
--- test_mul
add64 r10, 0
mov r0, 0x40000001
mov r1, 4
mul32 r0, r1
exit
--- test_mul
add64 r10, 0
mov r0, 0x40000001
mul r0, 4
exit
--- test_mul
add64 r10, 0
mov r0, 0x40000001
mov r1, 4
mul r0, r1
exit
--- test_mul
add64 r10, 0
mov r0, -1
mul32 r0, 4
exit
--- test_div
add64 r10, 0
mov r0, 12
lddw r1, 0x100000004
div32 r0, r1
exit
--- test_div
add64 r10, 0
lddw r0, 0x10000000c
div32 r0, 4
exit
--- test_div
add64 r10, 0
lddw r0, 0x10000000c
mov r1, 4
div32 r0, r1
exit
--- test_div
add64 r10, 0
mov r0, 0xc
lsh r0, 32
div r0, 4
exit
--- test_div
add64 r10, 0
mov r0, 0xc
lsh r0, 32
mov r1, 4
div r0, r1
exit
--- test_div
add64 r10, 0
mov32 r0, 1
mov32 r1, 0
div r0, r1
exit
--- test_div
add64 r10, 0
mov32 r0, 1
mov32 r1, 0
div32 r0, r1
exit
--- test_mod
add64 r10, 0
mov32 r0, 5748
mod32 r0, 92
mov32 r1, 13
mod32 r0, r1
exit
--- test_mod
add64 r10, 0
lddw r0, 0x100000003
mod32 r0, 3
exit
--- test_mod
add64 r10, 0
mov32 r0, -1316649930
lsh r0, 32
or r0, 0x100dc5c8
mov32 r1, 0xdde263e
lsh r1, 32
or r1, 0x3cbef7f3
mod r0, r1
mod r0, 0x658f1778
exit
--- test_mod
add64 r10, 0
mov32 r0, 1
mov32 r1, 0
mod r0, r1
exit
--- test_mod
add64 r10, 0
mov32 r0, 1
mov32 r1, 0
mod32 r0, r1
exit
--- test_div_mod_full_width
mov64 r0, 1
lsh64 r0, 40
mov64 r1, 1
lsh64 r1, 33
div64 r0, r1
exit
--- test_div_mod_full_width
mov64 r0, 1
lsh64 r0, 40
add64 r0, 5
mov64 r1, 1
lsh64 r1, 33
mod64 r0, r1
exit
--- test_div_imm_extension
mov64 r0, -1
div64 r0, -1
mov64 r1, -1
div32 r1, -1
add64 r0, r1
exit
--- test_div_imm_extension
mov64 r0, -1
mod64 r0, -2
mov64 r1, -1
mod32 r1, -2
mov64 r2, 7
div64 r2, 2
add64 r0, r1
add64 r0, r2
exit
--- test_symbol_relocation
add64 r10, 64
mov64 r1, r10
add64 r1, -0x1
mov64 r2, 0x1
syscall bpf_syscall_string
mov64 r0, 0x0
exit
--- test_stack_gaps
stw [r10 + 8], 77
call function_foo
exit
function_foo:
ldxdw r0, [r10 - 4088]
exit
--- test_stack_gaps
stw [r10 - 8], 77
call function_foo
exit
function_foo:
ldxdw r0, [r10 - 4104]
exit
--- test_stack_gaps
stw [r10 + 8], 77
call function_foo
exit
function_foo:
ldxdw r0, [r10 - 4088]
exit
--- test_access_violation_handler_consumes_instruction_meter
add64 r10, 0
mov64 r6, r1
stb [r6], 7
ldxb r0, [r6]
stb [r6+1], 8
ldxb r1, [r6+1]
add64 r0, r1
exit
