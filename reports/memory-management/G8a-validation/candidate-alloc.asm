
/home/dan/orca/projects/language/target/memory-binaries-2026-10-18/pool-clear-candidate:     file format elf64-x86-64


Disassembly of section .text:

0000000000b73dd0 <badger_runtime::gc::alloc>:
  b73dd0:	55                   	push   %rbp
  b73dd1:	41 57                	push   %r15
  b73dd3:	41 56                	push   %r14
  b73dd5:	41 55                	push   %r13
  b73dd7:	41 54                	push   %r12
  b73dd9:	53                   	push   %rbx
  b73dda:	48 83 ec 58          	sub    $0x58,%rsp
  b73dde:	49 89 f4             	mov    %rsi,%r12
  b73de1:	48 89 fb             	mov    %rdi,%rbx
  b73de4:	8b 05 36 d5 ed 00    	mov    0xedd536(%rip),%eax        # 1a51320 <badger_runtime::task::chaos::CHAOS+0x10>
  b73dea:	85 c0                	test   %eax,%eax
  b73dec:	0f 85 bc 00 00 00    	jne    b73eae <badger_runtime::gc::alloc+0xde>
  b73df2:	48 83 3d 16 d5 ed 00 	cmpq   $0x0,0xedd516(%rip)        # 1a51310 <badger_runtime::task::chaos::CHAOS>
  b73df9:	00 
  b73dfa:	0f 84 c1 00 00 00    	je     b73ec1 <badger_runtime::gc::alloc+0xf1>
  b73e00:	8b 05 1a d5 ed 00    	mov    0xedd51a(%rip),%eax        # 1a51320 <badger_runtime::task::chaos::CHAOS+0x10>
  b73e06:	85 c0                	test   %eax,%eax
  b73e08:	0f 85 e7 05 00 00    	jne    b743f5 <badger_runtime::gc::alloc+0x625>
  b73e0e:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b73e15:	ff ff 
  b73e17:	48 85 c0             	test   %rax,%rax
  b73e1a:	75 38                	jne    b73e54 <badger_runtime::gc::alloc+0x84>
  b73e1c:	49 89 de             	mov    %rbx,%r14
  b73e1f:	80 3d ea d4 ed 00 00 	cmpb   $0x0,0xedd4ea(%rip)        # 1a51310 <badger_runtime::task::chaos::CHAOS>
  b73e26:	74 09                	je     b73e31 <badger_runtime::gc::alloc+0x61>
  b73e28:	48 8b 1d e9 d4 ed 00 	mov    0xedd4e9(%rip),%rbx        # 1a51318 <badger_runtime::task::chaos::CHAOS+0x8>
  b73e2f:	eb 02                	jmp    b73e33 <badger_runtime::gc::alloc+0x63>
  b73e31:	31 db                	xor    %ebx,%ebx
  b73e33:	ff 15 8f d5 ec 00    	call   *0xecd58f(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b73e39:	48 b9 03 ed 92 d1 32 	movabs $0xd1b54a32d192ed03,%rcx
  b73e40:	4a b5 d1 
  b73e43:	48 0f af c1          	imul   %rcx,%rax
  b73e47:	48 01 c8             	add    %rcx,%rax
  b73e4a:	48 31 d8             	xor    %rbx,%rax
  b73e4d:	48 83 c8 01          	or     $0x1,%rax
  b73e51:	4c 89 f3             	mov    %r14,%rbx
  b73e54:	48 b9 15 7c 4a 7f b9 	movabs $0x9e3779b97f4a7c15,%rcx
  b73e5b:	79 37 9e 
  b73e5e:	48 01 c1             	add    %rax,%rcx
  b73e61:	64 48 89 0c 25 28 fb 	mov    %rcx,%fs:0xfffffffffffffb28
  b73e68:	ff ff 
  b73e6a:	48 89 c8             	mov    %rcx,%rax
  b73e6d:	48 c1 e8 1e          	shr    $0x1e,%rax
  b73e71:	48 31 c8             	xor    %rcx,%rax
  b73e74:	48 b9 b9 e5 e4 1c 6d 	movabs $0xbf58476d1ce4e5b9,%rcx
  b73e7b:	47 58 bf 
  b73e7e:	48 0f af c8          	imul   %rax,%rcx
  b73e82:	48 89 c8             	mov    %rcx,%rax
  b73e85:	48 c1 e8 1b          	shr    $0x1b,%rax
  b73e89:	48 31 c8             	xor    %rcx,%rax
  b73e8c:	48 b9 eb 11 31 13 bb 	movabs $0x94d049bb133111eb,%rcx
  b73e93:	49 d0 94 
  b73e96:	48 0f af c8          	imul   %rax,%rcx
  b73e9a:	48 89 c8             	mov    %rcx,%rax
  b73e9d:	48 c1 e8 1f          	shr    $0x1f,%rax
  b73ea1:	31 c8                	xor    %ecx,%eax
  b73ea3:	a9 ff 01 00 00       	test   $0x1ff,%eax
  b73ea8:	40 0f 94 c5          	sete   %bpl
  b73eac:	eb 15                	jmp    b73ec3 <badger_runtime::gc::alloc+0xf3>
  b73eae:	e8 9f 1f f5 ff       	call   ac5e52 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b73eb3:	48 83 3d 55 d4 ed 00 	cmpq   $0x0,0xedd455(%rip)        # 1a51310 <badger_runtime::task::chaos::CHAOS>
  b73eba:	00 
  b73ebb:	0f 85 3f ff ff ff    	jne    b73e00 <badger_runtime::gc::alloc+0x30>
  b73ec1:	31 ed                	xor    %ebp,%ebp
  b73ec3:	48 8b 05 06 3e ee 00 	mov    0xee3e06(%rip),%rax        # 1a57cd0 <badger_runtime::gc::ALLOCATED>
  b73eca:	4c 8b 3d ef d8 ec 00 	mov    0xecd8ef(%rip),%r15        # 1a417c0 <_DYNAMIC+0x3850>
  b73ed1:	49 8b 0f             	mov    (%r15),%rcx
  b73ed4:	48 01 c1             	add    %rax,%rcx
  b73ed7:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b73ede:	48 0f 43 c1          	cmovae %rcx,%rax
  b73ee2:	48 8b 0d 17 ad ed 00 	mov    0xedad17(%rip),%rcx        # 1a4ec00 <badger_runtime::gc::THRESHOLD>
  b73ee9:	48 39 c8             	cmp    %rcx,%rax
  b73eec:	77 20                	ja     b73f0e <badger_runtime::gc::alloc+0x13e>
  b73eee:	8b 05 94 d3 ed 00    	mov    0xedd394(%rip),%eax        # 1a51288 <badger_runtime::gc::stress::STRESS>
  b73ef4:	85 c0                	test   %eax,%eax
  b73ef6:	0f 85 15 05 00 00    	jne    b74411 <badger_runtime::gc::alloc+0x641>
  b73efc:	0f b6 05 89 d3 ed 00 	movzbl 0xedd389(%rip),%eax        # 1a5128c <badger_runtime::gc::stress::STRESS+0x4>
  b73f03:	40 08 e8             	or     %bpl,%al
  b73f06:	3c 01                	cmp    $0x1,%al
  b73f08:	0f 85 fe 00 00 00    	jne    b7400c <badger_runtime::gc::alloc+0x23c>
  b73f0e:	48 8b 05 1b e4 ec 00 	mov    0xece41b(%rip),%rax        # 1a42330 <_DYNAMIC+0x43c0>
  b73f15:	48 8b 00             	mov    (%rax),%rax
  b73f18:	48 85 c0             	test   %rax,%rax
  b73f1b:	0f 85 eb 00 00 00    	jne    b7400c <badger_runtime::gc::alloc+0x23c>
  b73f21:	ff 15 a1 d4 ec 00    	call   *0xecd4a1(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b73f27:	48 83 f8 3f          	cmp    $0x3f,%rax
  b73f2b:	0f 87 4e 06 00 00    	ja     b7457f <badger_runtime::gc::alloc+0x7af>
  b73f31:	49 89 dd             	mov    %rbx,%r13
  b73f34:	48 c1 e0 06          	shl    $0x6,%rax
  b73f38:	48 8d 15 81 1d ee 00 	lea    0xee1d81(%rip),%rdx        # 1a55cc0 <badger_runtime::gc::DEBT>
  b73f3f:	31 c9                	xor    %ecx,%ecx
  b73f41:	48 87 0c 10          	xchg   %rcx,(%rax,%rdx,1)
  b73f45:	48 85 c9             	test   %rcx,%rcx
  b73f48:	74 08                	je     b73f52 <badger_runtime::gc::alloc+0x182>
  b73f4a:	f0 48 01 0d 7e 3d ee 	lock add %rcx,0xee3d7e(%rip)        # 1a57cd0 <badger_runtime::gc::ALLOCATED>
  b73f51:	00 
  b73f52:	48 8b 05 27 d0 ed 00 	mov    0xedd027(%rip),%rax        # 1a50f80 <badger_runtime::task::WORKERS>
  b73f59:	48 83 f8 02          	cmp    $0x2,%rax
  b73f5d:	72 19                	jb     b73f78 <badger_runtime::gc::alloc+0x1a8>
  b73f5f:	48 8b 1d 32 5e ee 00 	mov    0xee5e32(%rip),%rbx        # 1a59d98 <badger_runtime::task::OWNER>
  b73f66:	ff 15 5c d4 ec 00    	call   *0xecd45c(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b73f6c:	48 ff c0             	inc    %rax
  b73f6f:	48 39 c3             	cmp    %rax,%rbx
  b73f72:	0f 85 9d 03 00 00    	jne    b74315 <badger_runtime::gc::alloc+0x545>
  b73f78:	bf 01 00 00 00       	mov    $0x1,%edi
  b73f7d:	e8 1e 67 a7 00       	call   15ea6a0 <<std::sys::pal::unix::time::Timespec>::now>
  b73f82:	49 89 c7             	mov    %rax,%r15
  b73f85:	89 d5                	mov    %edx,%ebp
  b73f87:	ff 15 8b e3 ec 00    	call   *0xece38b(%rip)        # 1a42318 <_DYNAMIC+0x43a8>
  b73f8d:	e8 ae aa ff ff       	call   b6ea40 <badger_runtime::gc::collect_inner>
  b73f92:	45 31 f6             	xor    %r14d,%r14d
  b73f95:	31 c0                	xor    %eax,%eax
  b73f97:	86 05 f3 5d ee 00    	xchg   %al,0xee5df3(%rip)        # 1a59d90 <badger_runtime::task::STOP>
  b73f9d:	bf 01 00 00 00       	mov    $0x1,%edi
  b73fa2:	e8 f9 66 a7 00       	call   15ea6a0 <<std::sys::pal::unix::time::Timespec>::now>
  b73fa7:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b73fac:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b73fb0:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b73fb5:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b73fb9:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b73fbe:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b73fc3:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b73fc8:	e8 03 66 a7 00       	call   15ea5d0 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b73fcd:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b73fd1:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b73fd8:	9a 3b 
  b73fda:	48 01 c1             	add    %rax,%rcx
  b73fdd:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b73fe2:	4c 0f 44 f1          	cmove  %rcx,%r14
  b73fe6:	48 8b 05 bb b1 ec 00 	mov    0xecb1bb(%rip),%rax        # 1a3f1a8 <_DYNAMIC+0x1238>
  b73fed:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b73ff4:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b73ffb:	49 39 ce             	cmp    %rcx,%r14
  b73ffe:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b74002:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b74009:	4c 89 eb             	mov    %r13,%rbx
  b7400c:	ff 15 b6 d3 ec 00    	call   *0xecd3b6(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b74012:	49 89 c7             	mov    %rax,%r15
  b74015:	49 83 fc 09          	cmp    $0x9,%r12
  b74019:	bd 08 00 00 00       	mov    $0x8,%ebp
  b7401e:	4c 89 64 24 20       	mov    %r12,0x20(%rsp)
  b74023:	49 0f 43 ec          	cmovae %r12,%rbp
  b74027:	48 8d 45 1f          	lea    0x1f(%rbp),%rax
  b7402b:	49 89 c5             	mov    %rax,%r13
  b7402e:	49 83 e5 f0          	and    $0xfffffffffffffff0,%r13
  b74032:	49 81 fd 00 02 00 00 	cmp    $0x200,%r13
  b74039:	76 40                	jbe    b7407b <badger_runtime::gc::alloc+0x2ab>
  b7403b:	48 83 c5 10          	add    $0x10,%rbp
  b7403f:	48 b8 f9 ff ff ff ff 	movabs $0x7ffffffffffffff9,%rax
  b74046:	ff ff 7f 
  b74049:	48 39 c5             	cmp    %rax,%rbp
  b7404c:	0f 83 c2 04 00 00    	jae    b74514 <badger_runtime::gc::alloc+0x744>
  b74052:	48 83 fd 07          	cmp    $0x7,%rbp
  b74056:	0f 86 8b 00 00 00    	jbe    b740e7 <badger_runtime::gc::alloc+0x317>
  b7405c:	be 01 00 00 00       	mov    $0x1,%esi
  b74061:	48 89 ef             	mov    %rbp,%rdi
  b74064:	ff 15 36 a1 ec 00    	call   *0xeca136(%rip)        # 1a3e1a0 <calloc@GLIBC_2.2.5>
  b7406a:	49 89 c4             	mov    %rax,%r12
  b7406d:	4d 85 e4             	test   %r12,%r12
  b74070:	0f 85 b5 01 00 00    	jne    b7422b <badger_runtime::gc::alloc+0x45b>
  b74076:	e9 ec 04 00 00       	jmp    b74567 <badger_runtime::gc::alloc+0x797>
  b7407b:	49 83 ff 40          	cmp    $0x40,%r15
  b7407f:	0f 83 b4 04 00 00    	jae    b74539 <badger_runtime::gc::alloc+0x769>
  b74085:	48 c1 e8 04          	shr    $0x4,%rax
  b74089:	48 8d 48 ff          	lea    -0x1(%rax),%rcx
  b7408d:	48 83 f9 20          	cmp    $0x20,%rcx
  b74091:	0f 83 b7 04 00 00    	jae    b7454e <badger_runtime::gc::alloc+0x77e>
  b74097:	49 69 cf 30 01 00 00 	imul   $0x130,%r15,%rcx
  b7409e:	48 8d 2d 53 53 ed 00 	lea    0xed5353(%rip),%rbp        # 1a493f8 <badger_runtime::gc::POOLS>
  b740a5:	48 01 cd             	add    %rcx,%rbp
  b740a8:	4c 8b 64 c5 10       	mov    0x10(%rbp,%rax,8),%r12
  b740ad:	4d 85 e4             	test   %r12,%r12
  b740b0:	74 7a                	je     b7412c <badger_runtime::gc::alloc+0x35c>
  b740b2:	49 8b 0c 24          	mov    (%r12),%rcx
  b740b6:	48 89 4c c5 10       	mov    %rcx,0x10(%rbp,%rax,8)
  b740bb:	49 83 fd 20          	cmp    $0x20,%r13
  b740bf:	0f 84 a1 01 00 00    	je     b74266 <badger_runtime::gc::alloc+0x496>
  b740c5:	49 83 fd 30          	cmp    $0x30,%r13
  b740c9:	0f 84 8c 01 00 00    	je     b7425b <badger_runtime::gc::alloc+0x48b>
  b740cf:	49 83 fd 40          	cmp    $0x40,%r13
  b740d3:	0f 85 98 01 00 00    	jne    b74271 <badger_runtime::gc::alloc+0x4a1>
  b740d9:	0f 57 c0             	xorps  %xmm0,%xmm0
  b740dc:	41 0f 11 44 24 30    	movups %xmm0,0x30(%r12)
  b740e2:	e9 77 01 00 00       	jmp    b7425e <badger_runtime::gc::alloc+0x48e>
  b740e7:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b740ee:	00 00 
  b740f0:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b740f5:	be 08 00 00 00       	mov    $0x8,%esi
  b740fa:	48 89 ea             	mov    %rbp,%rdx
  b740fd:	ff 15 7d a0 ec 00    	call   *0xeca07d(%rip)        # 1a3e180 <posix_memalign@GLIBC_2.2.5>
  b74103:	85 c0                	test   %eax,%eax
  b74105:	0f 85 5c 04 00 00    	jne    b74567 <badger_runtime::gc::alloc+0x797>
  b7410b:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b74110:	4d 85 e4             	test   %r12,%r12
  b74113:	0f 84 4e 04 00 00    	je     b74567 <badger_runtime::gc::alloc+0x797>
  b74119:	4c 89 e7             	mov    %r12,%rdi
  b7411c:	31 f6                	xor    %esi,%esi
  b7411e:	48 89 ea             	mov    %rbp,%rdx
  b74121:	ff 15 81 a0 ec 00    	call   *0xeca081(%rip)        # 1a3e1a8 <memset@GLIBC_2.2.5>
  b74127:	e9 ff 00 00 00       	jmp    b7422b <badger_runtime::gc::alloc+0x45b>
  b7412c:	4c 8b b5 20 01 00 00 	mov    0x120(%rbp),%r14
  b74133:	4d 39 ee             	cmp    %r13,%r14
  b74136:	0f 83 ca 00 00 00    	jae    b74206 <badger_runtime::gc::alloc+0x436>
  b7413c:	48 83 7d 10 00       	cmpq   $0x0,0x10(%rbp)
  b74141:	41 be 00 00 04 00    	mov    $0x40000,%r14d
  b74147:	b8 00 00 20 00       	mov    $0x200000,%eax
  b7414c:	4c 0f 45 f0          	cmovne %rax,%r14
  b74150:	b9 10 00 00 00       	mov    $0x10,%ecx
  b74155:	48 0f 45 c8          	cmovne %rax,%rcx
  b74159:	48 89 4c 24 48       	mov    %rcx,0x48(%rsp)
  b7415e:	48 89 5c 24 50       	mov    %rbx,0x50(%rsp)
  b74163:	0f 84 56 02 00 00    	je     b743bf <badger_runtime::gc::alloc+0x5ef>
  b74169:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b74170:	00 00 
  b74172:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b74177:	be 00 00 20 00       	mov    $0x200000,%esi
  b7417c:	ba 00 00 20 00       	mov    $0x200000,%edx
  b74181:	ff 15 f9 9f ec 00    	call   *0xec9ff9(%rip)        # 1a3e180 <posix_memalign@GLIBC_2.2.5>
  b74187:	85 c0                	test   %eax,%eax
  b74189:	0f 85 d8 03 00 00    	jne    b74567 <badger_runtime::gc::alloc+0x797>
  b7418f:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b74194:	4d 85 e4             	test   %r12,%r12
  b74197:	0f 84 ca 03 00 00    	je     b74567 <badger_runtime::gc::alloc+0x797>
  b7419d:	ba 00 00 20 00       	mov    $0x200000,%edx
  b741a2:	4c 89 e7             	mov    %r12,%rdi
  b741a5:	31 f6                	xor    %esi,%esi
  b741a7:	ff 15 fb 9f ec 00    	call   *0xec9ffb(%rip)        # 1a3e1a8 <memset@GLIBC_2.2.5>
  b741ad:	be 00 00 20 00       	mov    $0x200000,%esi
  b741b2:	4c 89 e7             	mov    %r12,%rdi
  b741b5:	ba 0e 00 00 00       	mov    $0xe,%edx
  b741ba:	ff 15 78 e1 ec 00    	call   *0xece178(%rip)        # 1a42338 <madvise@GLIBC_2.2.5>
  b741c0:	48 8b 5d 10          	mov    0x10(%rbp),%rbx
  b741c4:	48 3b 5d 00          	cmp    0x0(%rbp),%rbx
  b741c8:	0f 84 19 02 00 00    	je     b743e7 <badger_runtime::gc::alloc+0x617>
  b741ce:	48 8b 45 08          	mov    0x8(%rbp),%rax
  b741d2:	48 8d 0c 5b          	lea    (%rbx,%rbx,2),%rcx
  b741d6:	4c 89 24 c8          	mov    %r12,(%rax,%rcx,8)
  b741da:	4c 89 74 c8 08       	mov    %r14,0x8(%rax,%rcx,8)
  b741df:	48 8b 54 24 48       	mov    0x48(%rsp),%rdx
  b741e4:	48 89 54 c8 10       	mov    %rdx,0x10(%rax,%rcx,8)
  b741e9:	48 ff c3             	inc    %rbx
  b741ec:	48 89 5d 10          	mov    %rbx,0x10(%rbp)
  b741f0:	4c 89 a5 28 01 00 00 	mov    %r12,0x128(%rbp)
  b741f7:	f0 4c 01 35 a9 1a ee 	lock add %r14,0xee1aa9(%rip)        # 1a55ca8 <badger_runtime::gc::POOL_RESERVED>
  b741fe:	00 
  b741ff:	48 8b 5c 24 50       	mov    0x50(%rsp),%rbx
  b74204:	eb 07                	jmp    b7420d <badger_runtime::gc::alloc+0x43d>
  b74206:	4c 8b a5 18 01 00 00 	mov    0x118(%rbp),%r12
  b7420d:	4b 8d 04 2c          	lea    (%r12,%r13,1),%rax
  b74211:	48 89 85 18 01 00 00 	mov    %rax,0x118(%rbp)
  b74218:	4d 29 ee             	sub    %r13,%r14
  b7421b:	4c 89 b5 20 01 00 00 	mov    %r14,0x120(%rbp)
  b74222:	4d 85 e4             	test   %r12,%r12
  b74225:	0f 84 3c 03 00 00    	je     b74567 <badger_runtime::gc::alloc+0x797>
  b7422b:	49 89 1c 24          	mov    %rbx,(%r12)
  b7422f:	4c 8b 6c 24 20       	mov    0x20(%rsp),%r13
  b74234:	4c 89 e8             	mov    %r13,%rax
  b74237:	48 c1 e0 20          	shl    $0x20,%rax
  b7423b:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b74240:	49 83 ff 40          	cmp    $0x40,%r15
  b74244:	72 4e                	jb     b74294 <badger_runtime::gc::alloc+0x4c4>
  b74246:	48 8d 15 4b dc e4 00 	lea    0xe4dc4b(%rip),%rdx        # 19c1e98 <anon.2b1c4e02011fd073cb9c1029406e50fd.67.llvm.13773802233634508896+0xef0>
  b7424d:	be 40 00 00 00       	mov    $0x40,%esi
  b74252:	4c 89 ff             	mov    %r15,%rdi
  b74255:	ff 15 75 9f ec 00    	call   *0xec9f75(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b7425b:	0f 57 c0             	xorps  %xmm0,%xmm0
  b7425e:	41 0f 11 44 24 20    	movups %xmm0,0x20(%r12)
  b74264:	eb 03                	jmp    b74269 <badger_runtime::gc::alloc+0x499>
  b74266:	0f 57 c0             	xorps  %xmm0,%xmm0
  b74269:	41 0f 11 44 24 10    	movups %xmm0,0x10(%r12)
  b7426f:	eb 0e                	jmp    b7427f <badger_runtime::gc::alloc+0x4af>
  b74271:	4c 89 e7             	mov    %r12,%rdi
  b74274:	31 f6                	xor    %esi,%esi
  b74276:	4c 89 ea             	mov    %r13,%rdx
  b74279:	ff 15 29 9f ec 00    	call   *0xec9f29(%rip)        # 1a3e1a8 <memset@GLIBC_2.2.5>
  b7427f:	49 89 1c 24          	mov    %rbx,(%r12)
  b74283:	4c 8b 6c 24 20       	mov    0x20(%rsp),%r13
  b74288:	4c 89 e8             	mov    %r13,%rax
  b7428b:	48 c1 e0 20          	shl    $0x20,%rax
  b7428f:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b74294:	4b 8d 04 7f          	lea    (%r15,%r15,2),%rax
  b74298:	48 8b 0d e1 d3 ec 00 	mov    0xecd3e1(%rip),%rcx        # 1a41680 <_DYNAMIC+0x3710>
  b7429f:	4c 8d 34 c1          	lea    (%rcx,%rax,8),%r14
  b742a3:	48 8b 5c c1 10       	mov    0x10(%rcx,%rax,8),%rbx
  b742a8:	48 3b 1c c1          	cmp    (%rcx,%rax,8),%rbx
  b742ac:	0f 84 ff 00 00 00    	je     b743b1 <badger_runtime::gc::alloc+0x5e1>
  b742b2:	49 8b 46 08          	mov    0x8(%r14),%rax
  b742b6:	4c 89 24 d8          	mov    %r12,(%rax,%rbx,8)
  b742ba:	48 ff c3             	inc    %rbx
  b742bd:	49 89 5e 10          	mov    %rbx,0x10(%r14)
  b742c1:	49 c1 e7 06          	shl    $0x6,%r15
  b742c5:	48 8d 05 f4 29 ee 00 	lea    0xee29f4(%rip),%rax        # 1a56cc0 <badger_runtime::gc::COUNTS>
  b742cc:	49 89 1c 07          	mov    %rbx,(%r15,%rax,1)
  b742d0:	48 8d 0d e9 19 ee 00 	lea    0xee19e9(%rip),%rcx        # 1a55cc0 <badger_runtime::gc::DEBT>
  b742d7:	4a 8d 04 39          	lea    (%rcx,%r15,1),%rax
  b742db:	49 8b 0c 0f          	mov    (%r15,%rcx,1),%rcx
  b742df:	4c 01 e9             	add    %r13,%rcx
  b742e2:	48 83 c1 10          	add    $0x10,%rcx
  b742e6:	48 81 f9 ff ff 00 00 	cmp    $0xffff,%rcx
  b742ed:	76 11                	jbe    b74300 <badger_runtime::gc::alloc+0x530>
  b742ef:	48 c7 00 00 00 00 00 	movq   $0x0,(%rax)
  b742f6:	f0 48 01 0d d2 39 ee 	lock add %rcx,0xee39d2(%rip)        # 1a57cd0 <badger_runtime::gc::ALLOCATED>
  b742fd:	00 
  b742fe:	eb 03                	jmp    b74303 <badger_runtime::gc::alloc+0x533>
  b74300:	48 89 08             	mov    %rcx,(%rax)
  b74303:	4c 89 e0             	mov    %r12,%rax
  b74306:	48 83 c4 58          	add    $0x58,%rsp
  b7430a:	5b                   	pop    %rbx
  b7430b:	41 5c                	pop    %r12
  b7430d:	41 5d                	pop    %r13
  b7430f:	41 5e                	pop    %r14
  b74311:	41 5f                	pop    %r15
  b74313:	5d                   	pop    %rbp
  b74314:	c3                   	ret
  b74315:	48 8b 05 64 cc ed 00 	mov    0xedcc64(%rip),%rax        # 1a50f80 <badger_runtime::task::WORKERS>
  b7431c:	48 83 f8 02          	cmp    $0x2,%rax
  b74320:	72 2f                	jb     b74351 <badger_runtime::gc::alloc+0x581>
  b74322:	ff 15 a0 d0 ec 00    	call   *0xecd0a0(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b74328:	48 83 f8 40          	cmp    $0x40,%rax
  b7432c:	0f 83 62 02 00 00    	jae    b74594 <badger_runtime::gc::alloc+0x7c4>
  b74332:	48 c1 e0 07          	shl    $0x7,%rax
  b74336:	48 8d 0d 43 3a ee 00 	lea    0xee3a43(%rip),%rcx        # 1a57d80 <badger_runtime::task::IN_COMPILED>
  b7433d:	f0 48 ff 0c 08       	lock decq (%rax,%rcx,1)
  b74342:	e8 19 45 01 00       	call   b88860 <badger_runtime::task::chaos_jitter>
  b74347:	bf 05 00 00 00       	mov    $0x5,%edi
  b7434c:	e8 7f ee 01 00       	call   b931d0 <badger_runtime::task::lock_for>
  b74351:	48 8b 05 78 39 ee 00 	mov    0xee3978(%rip),%rax        # 1a57cd0 <badger_runtime::gc::ALLOCATED>
  b74358:	49 8b 0f             	mov    (%r15),%rcx
  b7435b:	48 01 c1             	add    %rax,%rcx
  b7435e:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b74365:	48 0f 43 c1          	cmovae %rcx,%rax
  b74369:	48 8b 0d 90 a8 ed 00 	mov    0xeda890(%rip),%rcx        # 1a4ec00 <badger_runtime::gc::THRESHOLD>
  b74370:	48 39 c8             	cmp    %rcx,%rax
  b74373:	4c 89 eb             	mov    %r13,%rbx
  b74376:	0f 87 c7 00 00 00    	ja     b74443 <badger_runtime::gc::alloc+0x673>
  b7437c:	8b 05 06 cf ed 00    	mov    0xedcf06(%rip),%eax        # 1a51288 <badger_runtime::gc::stress::STRESS>
  b74382:	85 c0                	test   %eax,%eax
  b74384:	0f 85 a3 00 00 00    	jne    b7442d <badger_runtime::gc::alloc+0x65d>
  b7438a:	40 0a 2d fb ce ed 00 	or     0xedcefb(%rip),%bpl        # 1a5128c <badger_runtime::gc::stress::STRESS+0x4>
  b74391:	40 80 fd 01          	cmp    $0x1,%bpl
  b74395:	0f 84 a8 00 00 00    	je     b74443 <badger_runtime::gc::alloc+0x673>
  b7439b:	48 8b 05 de cb ed 00 	mov    0xedcbde(%rip),%rax        # 1a50f80 <badger_runtime::task::WORKERS>
  b743a2:	48 83 f8 02          	cmp    $0x2,%rax
  b743a6:	0f 82 60 fc ff ff    	jb     b7400c <badger_runtime::gc::alloc+0x23c>
  b743ac:	e9 34 01 00 00       	jmp    b744e5 <badger_runtime::gc::alloc+0x715>
  b743b1:	4c 89 f7             	mov    %r14,%rdi
  b743b4:	ff 15 06 d0 ec 00    	call   *0xecd006(%rip)        # 1a413c0 <_DYNAMIC+0x3450>
  b743ba:	e9 f3 fe ff ff       	jmp    b742b2 <badger_runtime::gc::alloc+0x4e2>
  b743bf:	bf 00 00 04 00       	mov    $0x40000,%edi
  b743c4:	be 01 00 00 00       	mov    $0x1,%esi
  b743c9:	ff 15 d1 9d ec 00    	call   *0xec9dd1(%rip)        # 1a3e1a0 <calloc@GLIBC_2.2.5>
  b743cf:	48 85 c0             	test   %rax,%rax
  b743d2:	0f 84 8f 01 00 00    	je     b74567 <badger_runtime::gc::alloc+0x797>
  b743d8:	49 89 c4             	mov    %rax,%r12
  b743db:	31 db                	xor    %ebx,%ebx
  b743dd:	48 3b 5d 00          	cmp    0x0(%rbp),%rbx
  b743e1:	0f 85 e7 fd ff ff    	jne    b741ce <badger_runtime::gc::alloc+0x3fe>
  b743e7:	48 89 ef             	mov    %rbp,%rdi
  b743ea:	ff 15 10 d2 ec 00    	call   *0xecd210(%rip)        # 1a41600 <_DYNAMIC+0x3690>
  b743f0:	e9 d9 fd ff ff       	jmp    b741ce <badger_runtime::gc::alloc+0x3fe>
  b743f5:	e8 58 1a f5 ff       	call   ac5e52 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b743fa:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b74401:	ff ff 
  b74403:	48 85 c0             	test   %rax,%rax
  b74406:	0f 85 48 fa ff ff    	jne    b73e54 <badger_runtime::gc::alloc+0x84>
  b7440c:	e9 0b fa ff ff       	jmp    b73e1c <badger_runtime::gc::alloc+0x4c>
  b74411:	e8 79 23 f5 ff       	call   ac678f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b74416:	0f b6 05 6f ce ed 00 	movzbl 0xedce6f(%rip),%eax        # 1a5128c <badger_runtime::gc::stress::STRESS+0x4>
  b7441d:	40 08 e8             	or     %bpl,%al
  b74420:	3c 01                	cmp    $0x1,%al
  b74422:	0f 84 e6 fa ff ff    	je     b73f0e <badger_runtime::gc::alloc+0x13e>
  b74428:	e9 df fb ff ff       	jmp    b7400c <badger_runtime::gc::alloc+0x23c>
  b7442d:	e8 5d 23 f5 ff       	call   ac678f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b74432:	40 0a 2d 53 ce ed 00 	or     0xedce53(%rip),%bpl        # 1a5128c <badger_runtime::gc::stress::STRESS+0x4>
  b74439:	40 80 fd 01          	cmp    $0x1,%bpl
  b7443d:	0f 85 58 ff ff ff    	jne    b7439b <badger_runtime::gc::alloc+0x5cb>
  b74443:	bf 01 00 00 00       	mov    $0x1,%edi
  b74448:	e8 53 62 a7 00       	call   15ea6a0 <<std::sys::pal::unix::time::Timespec>::now>
  b7444d:	49 89 c7             	mov    %rax,%r15
  b74450:	89 d5                	mov    %edx,%ebp
  b74452:	ff 15 c0 de ec 00    	call   *0xecdec0(%rip)        # 1a42318 <_DYNAMIC+0x43a8>
  b74458:	e8 e3 a5 ff ff       	call   b6ea40 <badger_runtime::gc::collect_inner>
  b7445d:	45 31 f6             	xor    %r14d,%r14d
  b74460:	31 c0                	xor    %eax,%eax
  b74462:	86 05 28 59 ee 00    	xchg   %al,0xee5928(%rip)        # 1a59d90 <badger_runtime::task::STOP>
  b74468:	bf 01 00 00 00       	mov    $0x1,%edi
  b7446d:	e8 2e 62 a7 00       	call   15ea6a0 <<std::sys::pal::unix::time::Timespec>::now>
  b74472:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b74477:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b7447b:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b74480:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b74484:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b74489:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b7448e:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b74493:	e8 38 61 a7 00       	call   15ea5d0 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b74498:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b7449c:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b744a3:	9a 3b 
  b744a5:	48 01 c1             	add    %rax,%rcx
  b744a8:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b744ad:	4c 0f 44 f1          	cmove  %rcx,%r14
  b744b1:	48 8b 05 f0 ac ec 00 	mov    0xecacf0(%rip),%rax        # 1a3f1a8 <_DYNAMIC+0x1238>
  b744b8:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b744bf:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b744c6:	49 39 ce             	cmp    %rcx,%r14
  b744c9:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b744cd:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b744d4:	48 8b 05 a5 ca ed 00 	mov    0xedcaa5(%rip),%rax        # 1a50f80 <badger_runtime::task::WORKERS>
  b744db:	48 83 f8 02          	cmp    $0x2,%rax
  b744df:	0f 82 27 fb ff ff    	jb     b7400c <badger_runtime::gc::alloc+0x23c>
  b744e5:	ff 15 dd ce ec 00    	call   *0xeccedd(%rip)        # 1a413c8 <_DYNAMIC+0x3458>
  b744eb:	48 83 f8 40          	cmp    $0x40,%rax
  b744ef:	0f 83 b6 00 00 00    	jae    b745ab <badger_runtime::gc::alloc+0x7db>
  b744f5:	48 c1 e0 07          	shl    $0x7,%rax
  b744f9:	48 8d 0d 80 38 ee 00 	lea    0xee3880(%rip),%rcx        # 1a57d80 <badger_runtime::task::IN_COMPILED>
  b74500:	f0 48 ff 04 08       	lock incq (%rax,%rcx,1)
  b74505:	e8 d6 d6 01 00       	call   b91be0 <badger_runtime::task::unlock>
  b7450a:	e8 51 43 01 00       	call   b88860 <badger_runtime::task::chaos_jitter>
  b7450f:	e9 f8 fa ff ff       	jmp    b7400c <badger_runtime::gc::alloc+0x23c>
  b74514:	48 8d 3d c0 95 6f ff 	lea    -0x906a40(%rip),%rdi        # 26dadb <anon.2b1c4e02011fd073cb9c1029406e50fd.530.llvm.13773802233634508896>
  b7451b:	48 8d 0d a6 e4 e4 00 	lea    0xe4e4a6(%rip),%rcx        # 19c29c8 <anon.2b1c4e02011fd073cb9c1029406e50fd.560.llvm.13773802233634508896+0x160>
  b74522:	4c 8d 05 87 f4 e4 00 	lea    0xe4f487(%rip),%r8        # 19c39b0 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x1d0>
  b74529:	48 8d 54 24 07       	lea    0x7(%rsp),%rdx
  b7452e:	be 2b 00 00 00       	mov    $0x2b,%esi
  b74533:	ff 15 d7 9d ec 00    	call   *0xec9dd7(%rip)        # 1a3e310 <_DYNAMIC+0x3a0>
  b74539:	48 8d 15 00 f3 e4 00 	lea    0xe4f300(%rip),%rdx        # 19c3840 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x60>
  b74540:	be 40 00 00 00       	mov    $0x40,%esi
  b74545:	4c 89 ff             	mov    %r15,%rdi
  b74548:	ff 15 82 9c ec 00    	call   *0xec9c82(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b7454e:	48 8d 15 03 f3 e4 00 	lea    0xe4f303(%rip),%rdx        # 19c3858 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x78>
  b74555:	be 20 00 00 00       	mov    $0x20,%esi
  b7455a:	48 c7 c7 ff ff ff ff 	mov    $0xffffffffffffffff,%rdi
  b74561:	ff 15 69 9c ec 00    	call   *0xec9c69(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b74567:	48 8d 3d 72 b1 6f ff 	lea    -0x904e8e(%rip),%rdi        # 26f6e0 <anon.2b1c4e02011fd073cb9c1029406e50fd.1130.llvm.13773802233634508896+0xb>
  b7456e:	be 2b 00 00 00       	mov    $0x2b,%esi
  b74573:	ff 15 07 a4 ec 00    	call   *0xeca407(%rip)        # 1a3e980 <_DYNAMIC+0xa10>
  b74579:	ff 15 a1 a4 ec 00    	call   *0xeca4a1(%rip)        # 1a3ea20 <_DYNAMIC+0xab0>
  b7457f:	48 8d 15 ea f2 e4 00 	lea    0xe4f2ea(%rip),%rdx        # 19c3870 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x90>
  b74586:	be 40 00 00 00       	mov    $0x40,%esi
  b7458b:	48 89 c7             	mov    %rax,%rdi
  b7458e:	ff 15 3c 9c ec 00    	call   *0xec9c3c(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b74594:	48 8d 15 65 f7 e4 00 	lea    0xe4f765(%rip),%rdx        # 19c3d00 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x520>
  b7459b:	be 40 00 00 00       	mov    $0x40,%esi
  b745a0:	48 89 c7             	mov    %rax,%rdi
  b745a3:	ff 15 27 9c ec 00    	call   *0xec9c27(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b745a9:	eb 15                	jmp    b745c0 <badger_runtime::gc::alloc+0x7f0>
  b745ab:	48 8d 15 4e f7 e4 00 	lea    0xe4f74e(%rip),%rdx        # 19c3d00 <anon.2b1c4e02011fd073cb9c1029406e50fd.1084.llvm.13773802233634508896+0x520>
  b745b2:	be 40 00 00 00       	mov    $0x40,%esi
  b745b7:	48 89 c7             	mov    %rax,%rdi
  b745ba:	ff 15 10 9c ec 00    	call   *0xec9c10(%rip)        # 1a3e1d0 <_DYNAMIC+0x260>
  b745c0:	0f 0b                	ud2
  b745c2:	ff 15 10 a3 ec 00    	call   *0xeca310(%rip)        # 1a3e8d8 <_DYNAMIC+0x968>
  b745c8:	ff 15 0a a3 ec 00    	call   *0xeca30a(%rip)        # 1a3e8d8 <_DYNAMIC+0x968>
