
/home/dan/orca/projects/language/target/memory-binaries-2026-10-18/allocation-paths-ordinary:     file format elf64-x86-64


Disassembly of section .text:

0000000000b73dd0 <badger_runtime::gc::alloc>:
  b73dd0:	55                   	push   %rbp
  b73dd1:	41 57                	push   %r15
  b73dd3:	41 56                	push   %r14
  b73dd5:	41 55                	push   %r13
  b73dd7:	41 54                	push   %r12
  b73dd9:	53                   	push   %rbx
  b73dda:	48 83 ec 58          	sub    $0x58,%rsp
  b73dde:	48 89 f3             	mov    %rsi,%rbx
  b73de1:	49 89 fe             	mov    %rdi,%r14
  b73de4:	8b 05 b6 d4 ed 00    	mov    0xedd4b6(%rip),%eax        # 1a512a0 <badger_runtime::task::chaos::CHAOS+0x10>
  b73dea:	85 c0                	test   %eax,%eax
  b73dec:	0f 85 bd 00 00 00    	jne    b73eaf <badger_runtime::gc::alloc+0xdf>
  b73df2:	48 83 3d 96 d4 ed 00 	cmpq   $0x0,0xedd496(%rip)        # 1a51290 <badger_runtime::task::chaos::CHAOS>
  b73df9:	00 
  b73dfa:	0f 84 c2 00 00 00    	je     b73ec2 <badger_runtime::gc::alloc+0xf2>
  b73e00:	8b 05 9a d4 ed 00    	mov    0xedd49a(%rip),%eax        # 1a512a0 <badger_runtime::task::chaos::CHAOS+0x10>
  b73e06:	85 c0                	test   %eax,%eax
  b73e08:	0f 85 99 05 00 00    	jne    b743a7 <badger_runtime::gc::alloc+0x5d7>
  b73e0e:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b73e15:	ff ff 
  b73e17:	48 85 c0             	test   %rax,%rax
  b73e1a:	75 39                	jne    b73e55 <badger_runtime::gc::alloc+0x85>
  b73e1c:	4d 89 f7             	mov    %r14,%r15
  b73e1f:	80 3d 6a d4 ed 00 00 	cmpb   $0x0,0xedd46a(%rip)        # 1a51290 <badger_runtime::task::chaos::CHAOS>
  b73e26:	74 09                	je     b73e31 <badger_runtime::gc::alloc+0x61>
  b73e28:	4c 8b 35 69 d4 ed 00 	mov    0xedd469(%rip),%r14        # 1a51298 <badger_runtime::task::chaos::CHAOS+0x8>
  b73e2f:	eb 03                	jmp    b73e34 <badger_runtime::gc::alloc+0x64>
  b73e31:	45 31 f6             	xor    %r14d,%r14d
  b73e34:	ff 15 4e d5 ec 00    	call   *0xecd54e(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b73e3a:	48 b9 03 ed 92 d1 32 	movabs $0xd1b54a32d192ed03,%rcx
  b73e41:	4a b5 d1 
  b73e44:	48 0f af c1          	imul   %rcx,%rax
  b73e48:	48 01 c8             	add    %rcx,%rax
  b73e4b:	4c 31 f0             	xor    %r14,%rax
  b73e4e:	48 83 c8 01          	or     $0x1,%rax
  b73e52:	4d 89 fe             	mov    %r15,%r14
  b73e55:	48 b9 15 7c 4a 7f b9 	movabs $0x9e3779b97f4a7c15,%rcx
  b73e5c:	79 37 9e 
  b73e5f:	48 01 c1             	add    %rax,%rcx
  b73e62:	64 48 89 0c 25 28 fb 	mov    %rcx,%fs:0xfffffffffffffb28
  b73e69:	ff ff 
  b73e6b:	48 89 c8             	mov    %rcx,%rax
  b73e6e:	48 c1 e8 1e          	shr    $0x1e,%rax
  b73e72:	48 31 c8             	xor    %rcx,%rax
  b73e75:	48 b9 b9 e5 e4 1c 6d 	movabs $0xbf58476d1ce4e5b9,%rcx
  b73e7c:	47 58 bf 
  b73e7f:	48 0f af c8          	imul   %rax,%rcx
  b73e83:	48 89 c8             	mov    %rcx,%rax
  b73e86:	48 c1 e8 1b          	shr    $0x1b,%rax
  b73e8a:	48 31 c8             	xor    %rcx,%rax
  b73e8d:	48 b9 eb 11 31 13 bb 	movabs $0x94d049bb133111eb,%rcx
  b73e94:	49 d0 94 
  b73e97:	48 0f af c8          	imul   %rax,%rcx
  b73e9b:	48 89 c8             	mov    %rcx,%rax
  b73e9e:	48 c1 e8 1f          	shr    $0x1f,%rax
  b73ea2:	31 c8                	xor    %ecx,%eax
  b73ea4:	a9 ff 01 00 00       	test   $0x1ff,%eax
  b73ea9:	40 0f 94 c5          	sete   %bpl
  b73ead:	eb 15                	jmp    b73ec4 <badger_runtime::gc::alloc+0xf4>
  b73eaf:	e8 9e 1f f5 ff       	call   ac5e52 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b73eb4:	48 83 3d d4 d3 ed 00 	cmpq   $0x0,0xedd3d4(%rip)        # 1a51290 <badger_runtime::task::chaos::CHAOS>
  b73ebb:	00 
  b73ebc:	0f 85 3e ff ff ff    	jne    b73e00 <badger_runtime::gc::alloc+0x30>
  b73ec2:	31 ed                	xor    %ebp,%ebp
  b73ec4:	48 8b 05 85 3d ee 00 	mov    0xee3d85(%rip),%rax        # 1a57c50 <badger_runtime::gc::ALLOCATED>
  b73ecb:	4c 8b 3d ae d8 ec 00 	mov    0xecd8ae(%rip),%r15        # 1a41780 <_DYNAMIC+0x3850>
  b73ed2:	49 8b 0f             	mov    (%r15),%rcx
  b73ed5:	48 01 c1             	add    %rax,%rcx
  b73ed8:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b73edf:	48 0f 43 c1          	cmovae %rcx,%rax
  b73ee3:	48 8b 0d 96 ac ed 00 	mov    0xedac96(%rip),%rcx        # 1a4eb80 <badger_runtime::gc::THRESHOLD>
  b73eea:	48 39 c8             	cmp    %rcx,%rax
  b73eed:	77 20                	ja     b73f0f <badger_runtime::gc::alloc+0x13f>
  b73eef:	8b 05 13 d3 ed 00    	mov    0xedd313(%rip),%eax        # 1a51208 <badger_runtime::gc::stress::STRESS>
  b73ef5:	85 c0                	test   %eax,%eax
  b73ef7:	0f 85 c6 04 00 00    	jne    b743c3 <badger_runtime::gc::alloc+0x5f3>
  b73efd:	0f b6 05 08 d3 ed 00 	movzbl 0xedd308(%rip),%eax        # 1a5120c <badger_runtime::gc::stress::STRESS+0x4>
  b73f04:	40 08 e8             	or     %bpl,%al
  b73f07:	3c 01                	cmp    $0x1,%al
  b73f09:	0f 85 fe 00 00 00    	jne    b7400d <badger_runtime::gc::alloc+0x23d>
  b73f0f:	48 8b 05 da e3 ec 00 	mov    0xece3da(%rip),%rax        # 1a422f0 <_DYNAMIC+0x43c0>
  b73f16:	48 8b 00             	mov    (%rax),%rax
  b73f19:	48 85 c0             	test   %rax,%rax
  b73f1c:	0f 85 eb 00 00 00    	jne    b7400d <badger_runtime::gc::alloc+0x23d>
  b73f22:	ff 15 60 d4 ec 00    	call   *0xecd460(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b73f28:	48 83 f8 3f          	cmp    $0x3f,%rax
  b73f2c:	0f 87 17 06 00 00    	ja     b74549 <badger_runtime::gc::alloc+0x779>
  b73f32:	4d 89 f4             	mov    %r14,%r12
  b73f35:	48 c1 e0 06          	shl    $0x6,%rax
  b73f39:	48 8d 15 00 1d ee 00 	lea    0xee1d00(%rip),%rdx        # 1a55c40 <badger_runtime::gc::DEBT>
  b73f40:	31 c9                	xor    %ecx,%ecx
  b73f42:	48 87 0c 10          	xchg   %rcx,(%rax,%rdx,1)
  b73f46:	48 85 c9             	test   %rcx,%rcx
  b73f49:	74 08                	je     b73f53 <badger_runtime::gc::alloc+0x183>
  b73f4b:	f0 48 01 0d fd 3c ee 	lock add %rcx,0xee3cfd(%rip)        # 1a57c50 <badger_runtime::gc::ALLOCATED>
  b73f52:	00 
  b73f53:	48 8b 05 a6 cf ed 00 	mov    0xedcfa6(%rip),%rax        # 1a50f00 <badger_runtime::task::WORKERS>
  b73f5a:	48 83 f8 02          	cmp    $0x2,%rax
  b73f5e:	72 19                	jb     b73f79 <badger_runtime::gc::alloc+0x1a9>
  b73f60:	4c 8b 35 b1 5d ee 00 	mov    0xee5db1(%rip),%r14        # 1a59d18 <badger_runtime::task::OWNER>
  b73f67:	ff 15 1b d4 ec 00    	call   *0xecd41b(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b73f6d:	48 ff c0             	inc    %rax
  b73f70:	49 39 c6             	cmp    %rax,%r14
  b73f73:	0f 85 43 03 00 00    	jne    b742bc <badger_runtime::gc::alloc+0x4ec>
  b73f79:	bf 01 00 00 00       	mov    $0x1,%edi
  b73f7e:	e8 ed 66 a7 00       	call   15ea670 <<std::sys::pal::unix::time::Timespec>::now>
  b73f83:	49 89 c7             	mov    %rax,%r15
  b73f86:	89 d5                	mov    %edx,%ebp
  b73f88:	ff 15 4a e3 ec 00    	call   *0xece34a(%rip)        # 1a422d8 <_DYNAMIC+0x43a8>
  b73f8e:	e8 ad aa ff ff       	call   b6ea40 <badger_runtime::gc::collect_inner>
  b73f93:	45 31 f6             	xor    %r14d,%r14d
  b73f96:	31 c0                	xor    %eax,%eax
  b73f98:	86 05 72 5d ee 00    	xchg   %al,0xee5d72(%rip)        # 1a59d10 <badger_runtime::task::STOP>
  b73f9e:	bf 01 00 00 00       	mov    $0x1,%edi
  b73fa3:	e8 c8 66 a7 00       	call   15ea670 <<std::sys::pal::unix::time::Timespec>::now>
  b73fa8:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b73fad:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b73fb1:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b73fb6:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b73fba:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b73fbf:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b73fc4:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b73fc9:	e8 d2 65 a7 00       	call   15ea5a0 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b73fce:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b73fd2:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b73fd9:	9a 3b 
  b73fdb:	48 01 c1             	add    %rax,%rcx
  b73fde:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b73fe3:	4c 0f 44 f1          	cmove  %rcx,%r14
  b73fe7:	48 8b 05 7a b1 ec 00 	mov    0xecb17a(%rip),%rax        # 1a3f168 <_DYNAMIC+0x1238>
  b73fee:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b73ff5:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b73ffc:	49 39 ce             	cmp    %rcx,%r14
  b73fff:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b74003:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b7400a:	4d 89 e6             	mov    %r12,%r14
  b7400d:	ff 15 75 d3 ec 00    	call   *0xecd375(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b74013:	49 89 c7             	mov    %rax,%r15
  b74016:	48 83 fb 09          	cmp    $0x9,%rbx
  b7401a:	41 bd 08 00 00 00    	mov    $0x8,%r13d
  b74020:	4c 0f 43 eb          	cmovae %rbx,%r13
  b74024:	49 8d 45 1f          	lea    0x1f(%r13),%rax
  b74028:	48 89 c2             	mov    %rax,%rdx
  b7402b:	48 83 e2 f0          	and    $0xfffffffffffffff0,%rdx
  b7402f:	48 81 fa 00 02 00 00 	cmp    $0x200,%rdx
  b74036:	76 3c                	jbe    b74074 <badger_runtime::gc::alloc+0x2a4>
  b74038:	49 83 c5 10          	add    $0x10,%r13
  b7403c:	48 b8 f9 ff ff ff ff 	movabs $0x7ffffffffffffff9,%rax
  b74043:	ff ff 7f 
  b74046:	49 39 c5             	cmp    %rax,%r13
  b74049:	0f 83 8f 04 00 00    	jae    b744de <badger_runtime::gc::alloc+0x70e>
  b7404f:	49 83 fd 07          	cmp    $0x7,%r13
  b74053:	76 7f                	jbe    b740d4 <badger_runtime::gc::alloc+0x304>
  b74055:	be 01 00 00 00       	mov    $0x1,%esi
  b7405a:	4c 89 ef             	mov    %r13,%rdi
  b7405d:	ff 15 fd a0 ec 00    	call   *0xeca0fd(%rip)        # 1a3e160 <calloc@GLIBC_2.2.5>
  b74063:	49 89 c4             	mov    %rax,%r12
  b74066:	4d 85 e4             	test   %r12,%r12
  b74069:	0f 85 b2 01 00 00    	jne    b74221 <badger_runtime::gc::alloc+0x451>
  b7406f:	e9 bd 04 00 00       	jmp    b74531 <badger_runtime::gc::alloc+0x761>
  b74074:	49 83 ff 40          	cmp    $0x40,%r15
  b74078:	0f 83 85 04 00 00    	jae    b74503 <badger_runtime::gc::alloc+0x733>
  b7407e:	48 c1 e8 04          	shr    $0x4,%rax
  b74082:	48 8d 48 ff          	lea    -0x1(%rax),%rcx
  b74086:	48 83 f9 20          	cmp    $0x20,%rcx
  b7408a:	0f 83 88 04 00 00    	jae    b74518 <badger_runtime::gc::alloc+0x748>
  b74090:	49 69 cf 30 01 00 00 	imul   $0x130,%r15,%rcx
  b74097:	4c 8d 2d da 52 ed 00 	lea    0xed52da(%rip),%r13        # 1a49378 <badger_runtime::gc::POOLS>
  b7409e:	49 01 cd             	add    %rcx,%r13
  b740a1:	4d 8b 64 c5 10       	mov    0x10(%r13,%rax,8),%r12
  b740a6:	4d 85 e4             	test   %r12,%r12
  b740a9:	74 6e                	je     b74119 <badger_runtime::gc::alloc+0x349>
  b740ab:	49 8b 0c 24          	mov    (%r12),%rcx
  b740af:	49 89 4c c5 10       	mov    %rcx,0x10(%r13,%rax,8)
  b740b4:	4c 89 e7             	mov    %r12,%rdi
  b740b7:	31 f6                	xor    %esi,%esi
  b740b9:	ff 15 a9 a0 ec 00    	call   *0xeca0a9(%rip)        # 1a3e168 <memset@GLIBC_2.2.5>
  b740bf:	4d 89 34 24          	mov    %r14,(%r12)
  b740c3:	48 89 d8             	mov    %rbx,%rax
  b740c6:	48 c1 e0 20          	shl    $0x20,%rax
  b740ca:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b740cf:	e9 67 01 00 00       	jmp    b7423b <badger_runtime::gc::alloc+0x46b>
  b740d4:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b740db:	00 00 
  b740dd:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b740e2:	be 08 00 00 00       	mov    $0x8,%esi
  b740e7:	4c 89 ea             	mov    %r13,%rdx
  b740ea:	ff 15 50 a0 ec 00    	call   *0xeca050(%rip)        # 1a3e140 <posix_memalign@GLIBC_2.2.5>
  b740f0:	85 c0                	test   %eax,%eax
  b740f2:	0f 85 39 04 00 00    	jne    b74531 <badger_runtime::gc::alloc+0x761>
  b740f8:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b740fd:	4d 85 e4             	test   %r12,%r12
  b74100:	0f 84 2b 04 00 00    	je     b74531 <badger_runtime::gc::alloc+0x761>
  b74106:	4c 89 e7             	mov    %r12,%rdi
  b74109:	31 f6                	xor    %esi,%esi
  b7410b:	4c 89 ea             	mov    %r13,%rdx
  b7410e:	ff 15 54 a0 ec 00    	call   *0xeca054(%rip)        # 1a3e168 <memset@GLIBC_2.2.5>
  b74114:	e9 08 01 00 00       	jmp    b74221 <badger_runtime::gc::alloc+0x451>
  b74119:	49 8b ad 20 01 00 00 	mov    0x120(%r13),%rbp
  b74120:	48 39 d5             	cmp    %rdx,%rbp
  b74123:	0f 83 d3 00 00 00    	jae    b741fc <badger_runtime::gc::alloc+0x42c>
  b74129:	48 89 54 24 20       	mov    %rdx,0x20(%rsp)
  b7412e:	49 83 7d 10 00       	cmpq   $0x0,0x10(%r13)
  b74133:	bd 00 00 04 00       	mov    $0x40000,%ebp
  b74138:	b8 00 00 20 00       	mov    $0x200000,%eax
  b7413d:	48 0f 45 e8          	cmovne %rax,%rbp
  b74141:	b9 10 00 00 00       	mov    $0x10,%ecx
  b74146:	48 0f 45 c8          	cmovne %rax,%rcx
  b7414a:	48 89 4c 24 48       	mov    %rcx,0x48(%rsp)
  b7414f:	4c 89 74 24 50       	mov    %r14,0x50(%rsp)
  b74154:	0f 84 0c 02 00 00    	je     b74366 <badger_runtime::gc::alloc+0x596>
  b7415a:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b74161:	00 00 
  b74163:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b74168:	be 00 00 20 00       	mov    $0x200000,%esi
  b7416d:	ba 00 00 20 00       	mov    $0x200000,%edx
  b74172:	ff 15 c8 9f ec 00    	call   *0xec9fc8(%rip)        # 1a3e140 <posix_memalign@GLIBC_2.2.5>
  b74178:	85 c0                	test   %eax,%eax
  b7417a:	0f 85 b1 03 00 00    	jne    b74531 <badger_runtime::gc::alloc+0x761>
  b74180:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b74185:	4d 85 e4             	test   %r12,%r12
  b74188:	0f 84 a3 03 00 00    	je     b74531 <badger_runtime::gc::alloc+0x761>
  b7418e:	ba 00 00 20 00       	mov    $0x200000,%edx
  b74193:	4c 89 e7             	mov    %r12,%rdi
  b74196:	31 f6                	xor    %esi,%esi
  b74198:	ff 15 ca 9f ec 00    	call   *0xec9fca(%rip)        # 1a3e168 <memset@GLIBC_2.2.5>
  b7419e:	be 00 00 20 00       	mov    $0x200000,%esi
  b741a3:	4c 89 e7             	mov    %r12,%rdi
  b741a6:	ba 0e 00 00 00       	mov    $0xe,%edx
  b741ab:	ff 15 47 e1 ec 00    	call   *0xece147(%rip)        # 1a422f8 <madvise@GLIBC_2.2.5>
  b741b1:	4d 8b 75 10          	mov    0x10(%r13),%r14
  b741b5:	4d 3b 75 00          	cmp    0x0(%r13),%r14
  b741b9:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b741be:	0f 84 d0 01 00 00    	je     b74394 <badger_runtime::gc::alloc+0x5c4>
  b741c4:	49 8b 45 08          	mov    0x8(%r13),%rax
  b741c8:	4b 8d 0c 76          	lea    (%r14,%r14,2),%rcx
  b741cc:	4c 89 24 c8          	mov    %r12,(%rax,%rcx,8)
  b741d0:	48 89 6c c8 08       	mov    %rbp,0x8(%rax,%rcx,8)
  b741d5:	48 8b 74 24 48       	mov    0x48(%rsp),%rsi
  b741da:	48 89 74 c8 10       	mov    %rsi,0x10(%rax,%rcx,8)
  b741df:	49 ff c6             	inc    %r14
  b741e2:	4d 89 75 10          	mov    %r14,0x10(%r13)
  b741e6:	4d 89 a5 28 01 00 00 	mov    %r12,0x128(%r13)
  b741ed:	f0 48 01 2d 33 1a ee 	lock add %rbp,0xee1a33(%rip)        # 1a55c28 <badger_runtime::gc::POOL_RESERVED>
  b741f4:	00 
  b741f5:	4c 8b 74 24 50       	mov    0x50(%rsp),%r14
  b741fa:	eb 07                	jmp    b74203 <badger_runtime::gc::alloc+0x433>
  b741fc:	4d 8b a5 18 01 00 00 	mov    0x118(%r13),%r12
  b74203:	49 8d 04 14          	lea    (%r12,%rdx,1),%rax
  b74207:	49 89 85 18 01 00 00 	mov    %rax,0x118(%r13)
  b7420e:	48 29 d5             	sub    %rdx,%rbp
  b74211:	49 89 ad 20 01 00 00 	mov    %rbp,0x120(%r13)
  b74218:	4d 85 e4             	test   %r12,%r12
  b7421b:	0f 84 10 03 00 00    	je     b74531 <badger_runtime::gc::alloc+0x761>
  b74221:	4d 89 34 24          	mov    %r14,(%r12)
  b74225:	48 89 d8             	mov    %rbx,%rax
  b74228:	48 c1 e0 20          	shl    $0x20,%rax
  b7422c:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b74231:	49 83 ff 40          	cmp    $0x40,%r15
  b74235:	0f 83 8e 02 00 00    	jae    b744c9 <badger_runtime::gc::alloc+0x6f9>
  b7423b:	4b 8d 04 7f          	lea    (%r15,%r15,2),%rax
  b7423f:	48 8b 0d fa d3 ec 00 	mov    0xecd3fa(%rip),%rcx        # 1a41640 <_DYNAMIC+0x3710>
  b74246:	4c 8d 34 c1          	lea    (%rcx,%rax,8),%r14
  b7424a:	4c 8b 6c c1 10       	mov    0x10(%rcx,%rax,8),%r13
  b7424f:	4c 3b 2c c1          	cmp    (%rcx,%rax,8),%r13
  b74253:	0f 84 ff 00 00 00    	je     b74358 <badger_runtime::gc::alloc+0x588>
  b74259:	49 8b 46 08          	mov    0x8(%r14),%rax
  b7425d:	4e 89 24 e8          	mov    %r12,(%rax,%r13,8)
  b74261:	49 ff c5             	inc    %r13
  b74264:	4d 89 6e 10          	mov    %r13,0x10(%r14)
  b74268:	49 c1 e7 06          	shl    $0x6,%r15
  b7426c:	48 8d 05 cd 29 ee 00 	lea    0xee29cd(%rip),%rax        # 1a56c40 <badger_runtime::gc::COUNTS>
  b74273:	4d 89 2c 07          	mov    %r13,(%r15,%rax,1)
  b74277:	48 8d 0d c2 19 ee 00 	lea    0xee19c2(%rip),%rcx        # 1a55c40 <badger_runtime::gc::DEBT>
  b7427e:	4a 8d 04 39          	lea    (%rcx,%r15,1),%rax
  b74282:	49 8b 0c 0f          	mov    (%r15,%rcx,1),%rcx
  b74286:	48 01 d9             	add    %rbx,%rcx
  b74289:	48 83 c1 10          	add    $0x10,%rcx
  b7428d:	48 81 f9 ff ff 00 00 	cmp    $0xffff,%rcx
  b74294:	76 11                	jbe    b742a7 <badger_runtime::gc::alloc+0x4d7>
  b74296:	48 c7 00 00 00 00 00 	movq   $0x0,(%rax)
  b7429d:	f0 48 01 0d ab 39 ee 	lock add %rcx,0xee39ab(%rip)        # 1a57c50 <badger_runtime::gc::ALLOCATED>
  b742a4:	00 
  b742a5:	eb 03                	jmp    b742aa <badger_runtime::gc::alloc+0x4da>
  b742a7:	48 89 08             	mov    %rcx,(%rax)
  b742aa:	4c 89 e0             	mov    %r12,%rax
  b742ad:	48 83 c4 58          	add    $0x58,%rsp
  b742b1:	5b                   	pop    %rbx
  b742b2:	41 5c                	pop    %r12
  b742b4:	41 5d                	pop    %r13
  b742b6:	41 5e                	pop    %r14
  b742b8:	41 5f                	pop    %r15
  b742ba:	5d                   	pop    %rbp
  b742bb:	c3                   	ret
  b742bc:	48 8b 05 3d cc ed 00 	mov    0xedcc3d(%rip),%rax        # 1a50f00 <badger_runtime::task::WORKERS>
  b742c3:	48 83 f8 02          	cmp    $0x2,%rax
  b742c7:	4d 89 e6             	mov    %r12,%r14
  b742ca:	72 2f                	jb     b742fb <badger_runtime::gc::alloc+0x52b>
  b742cc:	ff 15 b6 d0 ec 00    	call   *0xecd0b6(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b742d2:	48 83 f8 40          	cmp    $0x40,%rax
  b742d6:	0f 83 82 02 00 00    	jae    b7455e <badger_runtime::gc::alloc+0x78e>
  b742dc:	48 c1 e0 07          	shl    $0x7,%rax
  b742e0:	48 8d 0d 19 3a ee 00 	lea    0xee3a19(%rip),%rcx        # 1a57d00 <badger_runtime::task::IN_COMPILED>
  b742e7:	f0 48 ff 0c 08       	lock decq (%rax,%rcx,1)
  b742ec:	e8 3f 45 01 00       	call   b88830 <badger_runtime::task::chaos_jitter>
  b742f1:	bf 05 00 00 00       	mov    $0x5,%edi
  b742f6:	e8 a5 ee 01 00       	call   b931a0 <badger_runtime::task::lock_for>
  b742fb:	48 8b 05 4e 39 ee 00 	mov    0xee394e(%rip),%rax        # 1a57c50 <badger_runtime::gc::ALLOCATED>
  b74302:	49 8b 0f             	mov    (%r15),%rcx
  b74305:	48 01 c1             	add    %rax,%rcx
  b74308:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b7430f:	48 0f 43 c1          	cmovae %rcx,%rax
  b74313:	48 8b 0d 66 a8 ed 00 	mov    0xeda866(%rip),%rcx        # 1a4eb80 <badger_runtime::gc::THRESHOLD>
  b7431a:	48 39 c8             	cmp    %rcx,%rax
  b7431d:	0f 87 d2 00 00 00    	ja     b743f5 <badger_runtime::gc::alloc+0x625>
  b74323:	8b 05 df ce ed 00    	mov    0xedcedf(%rip),%eax        # 1a51208 <badger_runtime::gc::stress::STRESS>
  b74329:	85 c0                	test   %eax,%eax
  b7432b:	0f 85 ae 00 00 00    	jne    b743df <badger_runtime::gc::alloc+0x60f>
  b74331:	40 0a 2d d4 ce ed 00 	or     0xedced4(%rip),%bpl        # 1a5120c <badger_runtime::gc::stress::STRESS+0x4>
  b74338:	40 80 fd 01          	cmp    $0x1,%bpl
  b7433c:	0f 84 b3 00 00 00    	je     b743f5 <badger_runtime::gc::alloc+0x625>
  b74342:	48 8b 05 b7 cb ed 00 	mov    0xedcbb7(%rip),%rax        # 1a50f00 <badger_runtime::task::WORKERS>
  b74349:	48 83 f8 02          	cmp    $0x2,%rax
  b7434d:	0f 82 ba fc ff ff    	jb     b7400d <badger_runtime::gc::alloc+0x23d>
  b74353:	e9 42 01 00 00       	jmp    b7449a <badger_runtime::gc::alloc+0x6ca>
  b74358:	4c 89 f7             	mov    %r14,%rdi
  b7435b:	ff 15 1f d0 ec 00    	call   *0xecd01f(%rip)        # 1a41380 <_DYNAMIC+0x3450>
  b74361:	e9 f3 fe ff ff       	jmp    b74259 <badger_runtime::gc::alloc+0x489>
  b74366:	bf 00 00 04 00       	mov    $0x40000,%edi
  b7436b:	be 01 00 00 00       	mov    $0x1,%esi
  b74370:	ff 15 ea 9d ec 00    	call   *0xec9dea(%rip)        # 1a3e160 <calloc@GLIBC_2.2.5>
  b74376:	48 85 c0             	test   %rax,%rax
  b74379:	0f 84 b2 01 00 00    	je     b74531 <badger_runtime::gc::alloc+0x761>
  b7437f:	49 89 c4             	mov    %rax,%r12
  b74382:	45 31 f6             	xor    %r14d,%r14d
  b74385:	4d 3b 75 00          	cmp    0x0(%r13),%r14
  b74389:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b7438e:	0f 85 30 fe ff ff    	jne    b741c4 <badger_runtime::gc::alloc+0x3f4>
  b74394:	4c 89 ef             	mov    %r13,%rdi
  b74397:	ff 15 23 d2 ec 00    	call   *0xecd223(%rip)        # 1a415c0 <_DYNAMIC+0x3690>
  b7439d:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b743a2:	e9 1d fe ff ff       	jmp    b741c4 <badger_runtime::gc::alloc+0x3f4>
  b743a7:	e8 a6 1a f5 ff       	call   ac5e52 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b743ac:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b743b3:	ff ff 
  b743b5:	48 85 c0             	test   %rax,%rax
  b743b8:	0f 85 97 fa ff ff    	jne    b73e55 <badger_runtime::gc::alloc+0x85>
  b743be:	e9 59 fa ff ff       	jmp    b73e1c <badger_runtime::gc::alloc+0x4c>
  b743c3:	e8 c7 23 f5 ff       	call   ac678f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b743c8:	0f b6 05 3d ce ed 00 	movzbl 0xedce3d(%rip),%eax        # 1a5120c <badger_runtime::gc::stress::STRESS+0x4>
  b743cf:	40 08 e8             	or     %bpl,%al
  b743d2:	3c 01                	cmp    $0x1,%al
  b743d4:	0f 84 35 fb ff ff    	je     b73f0f <badger_runtime::gc::alloc+0x13f>
  b743da:	e9 2e fc ff ff       	jmp    b7400d <badger_runtime::gc::alloc+0x23d>
  b743df:	e8 ab 23 f5 ff       	call   ac678f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b743e4:	40 0a 2d 21 ce ed 00 	or     0xedce21(%rip),%bpl        # 1a5120c <badger_runtime::gc::stress::STRESS+0x4>
  b743eb:	40 80 fd 01          	cmp    $0x1,%bpl
  b743ef:	0f 85 4d ff ff ff    	jne    b74342 <badger_runtime::gc::alloc+0x572>
  b743f5:	bf 01 00 00 00       	mov    $0x1,%edi
  b743fa:	e8 71 62 a7 00       	call   15ea670 <<std::sys::pal::unix::time::Timespec>::now>
  b743ff:	49 89 c7             	mov    %rax,%r15
  b74402:	89 d5                	mov    %edx,%ebp
  b74404:	ff 15 ce de ec 00    	call   *0xecdece(%rip)        # 1a422d8 <_DYNAMIC+0x43a8>
  b7440a:	e8 31 a6 ff ff       	call   b6ea40 <badger_runtime::gc::collect_inner>
  b7440f:	45 31 f6             	xor    %r14d,%r14d
  b74412:	31 c0                	xor    %eax,%eax
  b74414:	86 05 f6 58 ee 00    	xchg   %al,0xee58f6(%rip)        # 1a59d10 <badger_runtime::task::STOP>
  b7441a:	bf 01 00 00 00       	mov    $0x1,%edi
  b7441f:	e8 4c 62 a7 00       	call   15ea670 <<std::sys::pal::unix::time::Timespec>::now>
  b74424:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b74429:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b7442d:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b74432:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b74436:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b7443b:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b74440:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b74445:	e8 56 61 a7 00       	call   15ea5a0 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b7444a:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b7444e:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b74455:	9a 3b 
  b74457:	48 01 c1             	add    %rax,%rcx
  b7445a:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b7445f:	4c 0f 44 f1          	cmove  %rcx,%r14
  b74463:	48 8b 05 fe ac ec 00 	mov    0xecacfe(%rip),%rax        # 1a3f168 <_DYNAMIC+0x1238>
  b7446a:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b74471:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b74478:	49 39 ce             	cmp    %rcx,%r14
  b7447b:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b7447f:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b74486:	4d 89 e6             	mov    %r12,%r14
  b74489:	48 8b 05 70 ca ed 00 	mov    0xedca70(%rip),%rax        # 1a50f00 <badger_runtime::task::WORKERS>
  b74490:	48 83 f8 02          	cmp    $0x2,%rax
  b74494:	0f 82 73 fb ff ff    	jb     b7400d <badger_runtime::gc::alloc+0x23d>
  b7449a:	ff 15 e8 ce ec 00    	call   *0xeccee8(%rip)        # 1a41388 <_DYNAMIC+0x3458>
  b744a0:	48 83 f8 40          	cmp    $0x40,%rax
  b744a4:	0f 83 cb 00 00 00    	jae    b74575 <badger_runtime::gc::alloc+0x7a5>
  b744aa:	48 c1 e0 07          	shl    $0x7,%rax
  b744ae:	48 8d 0d 4b 38 ee 00 	lea    0xee384b(%rip),%rcx        # 1a57d00 <badger_runtime::task::IN_COMPILED>
  b744b5:	f0 48 ff 04 08       	lock incq (%rax,%rcx,1)
  b744ba:	e8 f1 d6 01 00       	call   b91bb0 <badger_runtime::task::unlock>
  b744bf:	e8 6c 43 01 00       	call   b88830 <badger_runtime::task::chaos_jitter>
  b744c4:	e9 44 fb ff ff       	jmp    b7400d <badger_runtime::gc::alloc+0x23d>
  b744c9:	48 8d 15 88 d9 e4 00 	lea    0xe4d988(%rip),%rdx        # 19c1e58 <anon.2b1c4e02011fd073cb9c1029406e50fd.67.llvm.8472078513322284042+0xef0>
  b744d0:	be 40 00 00 00       	mov    $0x40,%esi
  b744d5:	4c 89 ff             	mov    %r15,%rdi
  b744d8:	ff 15 b2 9c ec 00    	call   *0xec9cb2(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b744de:	48 8d 3d f6 95 6f ff 	lea    -0x906a0a(%rip),%rdi        # 26dadb <anon.2b1c4e02011fd073cb9c1029406e50fd.530.llvm.8472078513322284042>
  b744e5:	48 8d 0d 9c e4 e4 00 	lea    0xe4e49c(%rip),%rcx        # 19c2988 <anon.2b1c4e02011fd073cb9c1029406e50fd.560.llvm.8472078513322284042+0x160>
  b744ec:	4c 8d 05 7d f4 e4 00 	lea    0xe4f47d(%rip),%r8        # 19c3970 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x1d0>
  b744f3:	48 8d 54 24 07       	lea    0x7(%rsp),%rdx
  b744f8:	be 2b 00 00 00       	mov    $0x2b,%esi
  b744fd:	ff 15 cd 9d ec 00    	call   *0xec9dcd(%rip)        # 1a3e2d0 <_DYNAMIC+0x3a0>
  b74503:	48 8d 15 f6 f2 e4 00 	lea    0xe4f2f6(%rip),%rdx        # 19c3800 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x60>
  b7450a:	be 40 00 00 00       	mov    $0x40,%esi
  b7450f:	4c 89 ff             	mov    %r15,%rdi
  b74512:	ff 15 78 9c ec 00    	call   *0xec9c78(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b74518:	48 8d 15 f9 f2 e4 00 	lea    0xe4f2f9(%rip),%rdx        # 19c3818 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x78>
  b7451f:	be 20 00 00 00       	mov    $0x20,%esi
  b74524:	48 c7 c7 ff ff ff ff 	mov    $0xffffffffffffffff,%rdi
  b7452b:	ff 15 5f 9c ec 00    	call   *0xec9c5f(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b74531:	48 8d 3d a8 b1 6f ff 	lea    -0x904e58(%rip),%rdi        # 26f6e0 <anon.2b1c4e02011fd073cb9c1029406e50fd.1130.llvm.8472078513322284042+0xb>
  b74538:	be 2b 00 00 00       	mov    $0x2b,%esi
  b7453d:	ff 15 fd a3 ec 00    	call   *0xeca3fd(%rip)        # 1a3e940 <_DYNAMIC+0xa10>
  b74543:	ff 15 97 a4 ec 00    	call   *0xeca497(%rip)        # 1a3e9e0 <_DYNAMIC+0xab0>
  b74549:	48 8d 15 e0 f2 e4 00 	lea    0xe4f2e0(%rip),%rdx        # 19c3830 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x90>
  b74550:	be 40 00 00 00       	mov    $0x40,%esi
  b74555:	48 89 c7             	mov    %rax,%rdi
  b74558:	ff 15 32 9c ec 00    	call   *0xec9c32(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b7455e:	48 8d 15 5b f7 e4 00 	lea    0xe4f75b(%rip),%rdx        # 19c3cc0 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x520>
  b74565:	be 40 00 00 00       	mov    $0x40,%esi
  b7456a:	48 89 c7             	mov    %rax,%rdi
  b7456d:	ff 15 1d 9c ec 00    	call   *0xec9c1d(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b74573:	eb 15                	jmp    b7458a <badger_runtime::gc::alloc+0x7ba>
  b74575:	48 8d 15 44 f7 e4 00 	lea    0xe4f744(%rip),%rdx        # 19c3cc0 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.8472078513322284042+0x520>
  b7457c:	be 40 00 00 00       	mov    $0x40,%esi
  b74581:	48 89 c7             	mov    %rax,%rdi
  b74584:	ff 15 06 9c ec 00    	call   *0xec9c06(%rip)        # 1a3e190 <_DYNAMIC+0x260>
  b7458a:	0f 0b                	ud2
  b7458c:	ff 15 06 a3 ec 00    	call   *0xeca306(%rip)        # 1a3e898 <_DYNAMIC+0x968>
  b74592:	ff 15 00 a3 ec 00    	call   *0xeca300(%rip)        # 1a3e898 <_DYNAMIC+0x968>
