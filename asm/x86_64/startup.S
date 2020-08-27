##define BOOT_CR0 ( X86_CR0_PE \
#                 | X86_CR0_WP \
#                 | X86_CR0_PG )
#
##define BOOT_CR4 ( X86_CR4_DE         \
#                 | X86_CR4_PSE        \
#                 | X86_CR4_PAE        \
#                 | X86_CR4_PGE        \
#                 | X86_CR4_PCE        \
#                 | X86_CR4_OSFXSR     \
#                 | X86_CR4_OSXMMEXCPT )

.text
.code32

.data
.align 4096
.global ident_pt_l4
ident_pt_l4:
    .quad ident_pt_l3 + 0x67
    .rept 511
    .quad 0
    .endr
ident_pt_l3: //512 * 1gb hugepage ... it wouldn't work on processors which don't support 1GB Hugepage
    index = 0
    .rept 512
    .quad (index << 30) + 0x01e7
    index = index + 1
    .endr
/*
ident_pt_l3:
    .quad ident_pt_l2 + 0x67
    .rept 511
    .quad 0
    .endr
ident_pt_l2:
    index = 0
    .rept 512
    .quad (index << 21) + 0x1e7
    index = index + 1
    .endr
*/
gdt_desc:
    .short gdt_end - gdt - 1
    .long gdt

.align 8
gdt64_desc:
    .short gdt_end - gdt - 1
    .quad gdt

.align 8
gdt = . - 8
    #.quad 0x0000000000000000 # null descriptor
    .quad 0x00af9b000000ffff # 64-bit code segment
    .quad 0x00cf93000000ffff # 64-bit data segment
    .quad 0x00cf9b000000ffff # 32-bit code segment
gdt_end = .
.global gdt64_desc

.align 8
. = . + 4

.bss

.align 4096
. = . + 4096*32
init_stack_top = .

.text
.code32
.global entry32
entry32:
  mov $0x10, %eax
  mov %eax, %ds
  mov %eax, %es
  mov %eax, %fs
  mov %eax, %gs
  mov %eax, %ss
  ljmp $0x18, $1f
1:
  mov $0x000007b8, %eax  #DE PSE PAE PGE PCE OSFXSR OSXMMEXCPT
  mov %eax, %cr4

  lea ident_pt_l4, %eax
  mov %eax, %cr3

  # Set long mode
  mov $0xc0000080, %ecx
  mov $0x00000900, %eax
  xor %edx, %edx
  wrmsr # Write contents of EDX:EAX (0:to Model Specific Register specified by ECX register  
  
  mov $0x80010003, %eax  # PE,WP,PG, MP
  mov %eax, %cr0

  ljmpl $8, $startup64

.code64
.global entry64
entry64:
  mov %rsi, %rdi  # save boot_params to rdi
  lgdt gdt64_desc
  subq $8, %rsp
  movl $0x18, 4(%rsp)
  movl $entry32, %eax
  movl %eax, (%rsp)
  lret

.global startup64
startup64:
  push %rdi

  # lea .bss, %rdi
  # lea .edata, %rcx
  # sub %rdi, %rcx
  # xor %eax, %eax
  # rep stosb

  cli # mask interrupts

  pop %rdi
  lea init_stack_top, %rsp
  call start_rust # never return


.align 16
.global interrupt_handlers
interrupt_handlers:
index = 0
.rept 256
handler_start = .
.if index == 9 || 10 <= index && index <= 14 || index == 17
  .align 16
  cli
  push $index
  jmp interrupt_common
  .align 16
.else
  .align 16
  cli
  push $0
  push $index
  jmp interrupt_common
  .align 16
.endif
index = index + 1
.endr

interrupt_common:
  xchgq (%rsp), %rdi
  sub $256, %rsp
  movdqu %xmm15, (%rsp)
  movdqu %xmm14, 16(%rsp)
  movdqu %xmm13, 32(%rsp)
  movdqu %xmm12, 48(%rsp)
  movdqu %xmm11, 64(%rsp)
  movdqu %xmm10, 80(%rsp)
  movdqu %xmm9, 96(%rsp)
  movdqu %xmm8, 112(%rsp)
  movdqu %xmm7, 128(%rsp)
  movdqu %xmm6, 144(%rsp)
  movdqu %xmm5, 160(%rsp)
  movdqu %xmm4, 176(%rsp)
  movdqu %xmm3, 192(%rsp)
  movdqu %xmm2, 208(%rsp)
  movdqu %xmm1, 224(%rsp)
  movdqu %xmm0, 240(%rsp)
  pushq %r15
  pushq %r14
  pushq %r13
  pushq %r12
  pushq %r11
  pushq %r10
  pushq %r9
  pushq %r8
  pushq %rbp
  pushq %rsi
  pushq %rdx
  pushq %rcx
  pushq %rbx
  pushq %rax

  movq %rsp, %rsi
  
  pushq $0 # for alignment
  call interrupt_handler
  popq %rax # for alignment
  
  popq %rax
  popq %rbx
  popq %rcx
  popq %rdx
  popq %rsi
  popq %rbp
  popq %r8
  popq %r9
  popq %r10
  popq %r11
  popq %r12
  popq %r13
  popq %r14
  popq %r15
  movdqu (%rsp), %xmm15
  movdqu 16(%rsp), %xmm14
  movdqu 32(%rsp), %xmm13
  movdqu 48(%rsp), %xmm12
  movdqu 64(%rsp), %xmm11
  movdqu 80(%rsp), %xmm10
  movdqu 96(%rsp), %xmm9
  movdqu 112(%rsp), %xmm8
  movdqu 128(%rsp), %xmm7
  movdqu 144(%rsp), %xmm6
  movdqu 160(%rsp), %xmm5
  movdqu 176(%rsp), %xmm4
  movdqu 192(%rsp), %xmm3
  movdqu 208(%rsp), %xmm2
  movdqu 224(%rsp), %xmm1
  movdqu 240(%rsp), %xmm0
  add $256, %rsp
  popq %rdi

  addq $8, %rsp

  iretq
