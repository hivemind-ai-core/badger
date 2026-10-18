
/home/dan/orca/projects/language/target/memory-binaries-2026-10-18/hamt-init-candidate:     file format elf64-x86-64


Disassembly of section .text:

0000000000b745d0 <badger_runtime::gc::alloc>:
  b745d0:	55                   	push   %rbp
  b745d1:	41 57                	push   %r15
  b745d3:	41 56                	push   %r14
  b745d5:	41 55                	push   %r13
  b745d7:	41 54                	push   %r12
  b745d9:	53                   	push   %rbx
  b745da:	48 83 ec 58          	sub    $0x58,%rsp
  b745de:	48 89 f3             	mov    %rsi,%rbx
  b745e1:	49 89 fe             	mov    %rdi,%r14
  b745e4:	8b 05 b6 d4 ed 00    	mov    0xedd4b6(%rip),%eax        # 1a51aa0 <badger_runtime::task::chaos::CHAOS+0x10>
  b745ea:	85 c0                	test   %eax,%eax
  b745ec:	0f 85 bd 00 00 00    	jne    b746af <badger_runtime::gc::alloc+0xdf>
  b745f2:	48 83 3d 96 d4 ed 00 	cmpq   $0x0,0xedd496(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b745f9:	00 
  b745fa:	0f 84 c2 00 00 00    	je     b746c2 <badger_runtime::gc::alloc+0xf2>
  b74600:	8b 05 9a d4 ed 00    	mov    0xedd49a(%rip),%eax        # 1a51aa0 <badger_runtime::task::chaos::CHAOS+0x10>
  b74606:	85 c0                	test   %eax,%eax
  b74608:	0f 85 a6 05 00 00    	jne    b74bb4 <badger_runtime::gc::alloc+0x5e4>
  b7460e:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b74615:	ff ff 
  b74617:	48 85 c0             	test   %rax,%rax
  b7461a:	75 39                	jne    b74655 <badger_runtime::gc::alloc+0x85>
  b7461c:	4d 89 f7             	mov    %r14,%r15
  b7461f:	80 3d 6a d4 ed 00 00 	cmpb   $0x0,0xedd46a(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b74626:	74 09                	je     b74631 <badger_runtime::gc::alloc+0x61>
  b74628:	4c 8b 35 69 d4 ed 00 	mov    0xedd469(%rip),%r14        # 1a51a98 <badger_runtime::task::chaos::CHAOS+0x8>
  b7462f:	eb 03                	jmp    b74634 <badger_runtime::gc::alloc+0x64>
  b74631:	45 31 f6             	xor    %r14d,%r14d
  b74634:	ff 15 06 d5 ec 00    	call   *0xecd506(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b7463a:	48 b9 03 ed 92 d1 32 	movabs $0xd1b54a32d192ed03,%rcx
  b74641:	4a b5 d1 
  b74644:	48 0f af c1          	imul   %rcx,%rax
  b74648:	48 01 c8             	add    %rcx,%rax
  b7464b:	4c 31 f0             	xor    %r14,%rax
  b7464e:	48 83 c8 01          	or     $0x1,%rax
  b74652:	4d 89 fe             	mov    %r15,%r14
  b74655:	48 b9 15 7c 4a 7f b9 	movabs $0x9e3779b97f4a7c15,%rcx
  b7465c:	79 37 9e 
  b7465f:	48 01 c1             	add    %rax,%rcx
  b74662:	64 48 89 0c 25 28 fb 	mov    %rcx,%fs:0xfffffffffffffb28
  b74669:	ff ff 
  b7466b:	48 89 c8             	mov    %rcx,%rax
  b7466e:	48 c1 e8 1e          	shr    $0x1e,%rax
  b74672:	48 31 c8             	xor    %rcx,%rax
  b74675:	48 b9 b9 e5 e4 1c 6d 	movabs $0xbf58476d1ce4e5b9,%rcx
  b7467c:	47 58 bf 
  b7467f:	48 0f af c8          	imul   %rax,%rcx
  b74683:	48 89 c8             	mov    %rcx,%rax
  b74686:	48 c1 e8 1b          	shr    $0x1b,%rax
  b7468a:	48 31 c8             	xor    %rcx,%rax
  b7468d:	48 b9 eb 11 31 13 bb 	movabs $0x94d049bb133111eb,%rcx
  b74694:	49 d0 94 
  b74697:	48 0f af c8          	imul   %rax,%rcx
  b7469b:	48 89 c8             	mov    %rcx,%rax
  b7469e:	48 c1 e8 1f          	shr    $0x1f,%rax
  b746a2:	31 c8                	xor    %ecx,%eax
  b746a4:	a9 ff 01 00 00       	test   $0x1ff,%eax
  b746a9:	40 0f 94 c5          	sete   %bpl
  b746ad:	eb 15                	jmp    b746c4 <badger_runtime::gc::alloc+0xf4>
  b746af:	e8 1e 18 f5 ff       	call   ac5ed2 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b746b4:	48 83 3d d4 d3 ed 00 	cmpq   $0x0,0xedd3d4(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b746bb:	00 
  b746bc:	0f 85 3e ff ff ff    	jne    b74600 <badger_runtime::gc::alloc+0x30>
  b746c2:	31 ed                	xor    %ebp,%ebp
  b746c4:	48 8b 05 d5 e3 ec 00 	mov    0xece3d5(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b746cb:	48 8b 00             	mov    (%rax),%rax
  b746ce:	4c 8b 3d 6b d8 ec 00 	mov    0xecd86b(%rip),%r15        # 1a41f40 <_DYNAMIC+0x3850>
  b746d5:	49 8b 0f             	mov    (%r15),%rcx
  b746d8:	48 01 c1             	add    %rax,%rcx
  b746db:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b746e2:	48 0f 43 c1          	cmovae %rcx,%rax
  b746e6:	4c 8b 25 bb e3 ec 00 	mov    0xece3bb(%rip),%r12        # 1a42aa8 <_DYNAMIC+0x43b8>
  b746ed:	49 8b 0c 24          	mov    (%r12),%rcx
  b746f1:	48 39 c8             	cmp    %rcx,%rax
  b746f4:	77 20                	ja     b74716 <badger_runtime::gc::alloc+0x146>
  b746f6:	8b 05 0c d3 ed 00    	mov    0xedd30c(%rip),%eax        # 1a51a08 <badger_runtime::gc::stress::STRESS>
  b746fc:	85 c0                	test   %eax,%eax
  b746fe:	0f 85 cc 04 00 00    	jne    b74bd0 <badger_runtime::gc::alloc+0x600>
  b74704:	0f b6 05 01 d3 ed 00 	movzbl 0xedd301(%rip),%eax        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b7470b:	40 08 e8             	or     %bpl,%al
  b7470e:	3c 01                	cmp    $0x1,%al
  b74710:	0f 85 01 01 00 00    	jne    b74817 <badger_runtime::gc::alloc+0x247>
  b74716:	48 8b 05 93 e3 ec 00 	mov    0xece393(%rip),%rax        # 1a42ab0 <_DYNAMIC+0x43c0>
  b7471d:	48 8b 00             	mov    (%rax),%rax
  b74720:	48 85 c0             	test   %rax,%rax
  b74723:	0f 85 ee 00 00 00    	jne    b74817 <badger_runtime::gc::alloc+0x247>
  b74729:	ff 15 11 d4 ec 00    	call   *0xecd411(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b7472f:	48 83 f8 3f          	cmp    $0x3f,%rax
  b74733:	0f 87 1d 06 00 00    	ja     b74d56 <badger_runtime::gc::alloc+0x786>
  b74739:	4d 89 f5             	mov    %r14,%r13
  b7473c:	48 c1 e0 06          	shl    $0x6,%rax
  b74740:	48 8b 15 71 e3 ec 00 	mov    0xece371(%rip),%rdx        # 1a42ab8 <_DYNAMIC+0x43c8>
  b74747:	31 c9                	xor    %ecx,%ecx
  b74749:	48 87 0c 02          	xchg   %rcx,(%rdx,%rax,1)
  b7474d:	48 85 c9             	test   %rcx,%rcx
  b74750:	74 0b                	je     b7475d <badger_runtime::gc::alloc+0x18d>
  b74752:	48 8b 05 47 e3 ec 00 	mov    0xece347(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b74759:	f0 48 01 08          	lock add %rcx,(%rax)
  b7475d:	48 8b 05 9c cf ed 00 	mov    0xedcf9c(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b74764:	48 83 f8 02          	cmp    $0x2,%rax
  b74768:	72 19                	jb     b74783 <badger_runtime::gc::alloc+0x1b3>
  b7476a:	4c 8b 35 a7 5d ee 00 	mov    0xee5da7(%rip),%r14        # 1a5a518 <badger_runtime::task::OWNER>
  b74771:	ff 15 c9 d3 ec 00    	call   *0xecd3c9(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b74777:	48 ff c0             	inc    %rax
  b7477a:	49 39 c6             	cmp    %rax,%r14
  b7477d:	0f 85 46 03 00 00    	jne    b74ac9 <badger_runtime::gc::alloc+0x4f9>
  b74783:	bf 01 00 00 00       	mov    $0x1,%edi
  b74788:	e8 c3 66 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b7478d:	49 89 c7             	mov    %rax,%r15
  b74790:	89 d5                	mov    %edx,%ebp
  b74792:	ff 15 28 e3 ec 00    	call   *0xece328(%rip)        # 1a42ac0 <_DYNAMIC+0x43d0>
  b74798:	e8 73 aa ff ff       	call   b6f210 <badger_runtime::gc::collect_inner>
  b7479d:	45 31 f6             	xor    %r14d,%r14d
  b747a0:	31 c0                	xor    %eax,%eax
  b747a2:	86 05 68 5d ee 00    	xchg   %al,0xee5d68(%rip)        # 1a5a510 <badger_runtime::task::STOP>
  b747a8:	bf 01 00 00 00       	mov    $0x1,%edi
  b747ad:	e8 9e 66 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b747b2:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b747b7:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b747bb:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b747c0:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b747c4:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b747c9:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b747ce:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b747d3:	e8 a8 65 a7 00       	call   15ead80 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b747d8:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b747dc:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b747e3:	9a 3b 
  b747e5:	48 01 c1             	add    %rax,%rcx
  b747e8:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b747ed:	4c 0f 44 f1          	cmove  %rcx,%r14
  b747f1:	48 8b 05 30 b1 ec 00 	mov    0xecb130(%rip),%rax        # 1a3f928 <_DYNAMIC+0x1238>
  b747f8:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b747ff:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b74806:	49 39 ce             	cmp    %rcx,%r14
  b74809:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b7480d:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b74814:	4d 89 ee             	mov    %r13,%r14
  b74817:	ff 15 23 d3 ec 00    	call   *0xecd323(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b7481d:	49 89 c7             	mov    %rax,%r15
  b74820:	48 83 fb 09          	cmp    $0x9,%rbx
  b74824:	41 bd 08 00 00 00    	mov    $0x8,%r13d
  b7482a:	4c 0f 43 eb          	cmovae %rbx,%r13
  b7482e:	49 8d 45 1f          	lea    0x1f(%r13),%rax
  b74832:	48 89 c2             	mov    %rax,%rdx
  b74835:	48 83 e2 f0          	and    $0xfffffffffffffff0,%rdx
  b74839:	48 81 fa 00 02 00 00 	cmp    $0x200,%rdx
  b74840:	76 3c                	jbe    b7487e <badger_runtime::gc::alloc+0x2ae>
  b74842:	49 83 c5 10          	add    $0x10,%r13
  b74846:	48 b8 f9 ff ff ff ff 	movabs $0x7ffffffffffffff9,%rax
  b7484d:	ff ff 7f 
  b74850:	49 39 c5             	cmp    %rax,%r13
  b74853:	0f 83 92 04 00 00    	jae    b74ceb <badger_runtime::gc::alloc+0x71b>
  b74859:	49 83 fd 07          	cmp    $0x7,%r13
  b7485d:	76 7c                	jbe    b748db <badger_runtime::gc::alloc+0x30b>
  b7485f:	be 01 00 00 00       	mov    $0x1,%esi
  b74864:	4c 89 ef             	mov    %r13,%rdi
  b74867:	ff 15 b3 a0 ec 00    	call   *0xeca0b3(%rip)        # 1a3e920 <calloc@GLIBC_2.2.5>
  b7486d:	49 89 c4             	mov    %rax,%r12
  b74870:	4d 85 e4             	test   %r12,%r12
  b74873:	0f 85 b2 01 00 00    	jne    b74a2b <badger_runtime::gc::alloc+0x45b>
  b74879:	e9 c0 04 00 00       	jmp    b74d3e <badger_runtime::gc::alloc+0x76e>
  b7487e:	49 83 ff 40          	cmp    $0x40,%r15
  b74882:	0f 83 88 04 00 00    	jae    b74d10 <badger_runtime::gc::alloc+0x740>
  b74888:	48 c1 e8 04          	shr    $0x4,%rax
  b7488c:	48 8d 48 ff          	lea    -0x1(%rax),%rcx
  b74890:	48 83 f9 20          	cmp    $0x20,%rcx
  b74894:	0f 83 8b 04 00 00    	jae    b74d25 <badger_runtime::gc::alloc+0x755>
  b7489a:	4d 69 ef 30 01 00 00 	imul   $0x130,%r15,%r13
  b748a1:	4c 03 2d 58 d5 ec 00 	add    0xecd558(%rip),%r13        # 1a41e00 <_DYNAMIC+0x3710>
  b748a8:	4d 8b 64 c5 10       	mov    0x10(%r13,%rax,8),%r12
  b748ad:	4d 85 e4             	test   %r12,%r12
  b748b0:	74 6e                	je     b74920 <badger_runtime::gc::alloc+0x350>
  b748b2:	49 8b 0c 24          	mov    (%r12),%rcx
  b748b6:	49 89 4c c5 10       	mov    %rcx,0x10(%r13,%rax,8)
  b748bb:	4c 89 e7             	mov    %r12,%rdi
  b748be:	31 f6                	xor    %esi,%esi
  b748c0:	ff 15 62 a0 ec 00    	call   *0xeca062(%rip)        # 1a3e928 <memset@GLIBC_2.2.5>
  b748c6:	4d 89 34 24          	mov    %r14,(%r12)
  b748ca:	48 89 d8             	mov    %rbx,%rax
  b748cd:	48 c1 e0 20          	shl    $0x20,%rax
  b748d1:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b748d6:	e9 6a 01 00 00       	jmp    b74a45 <badger_runtime::gc::alloc+0x475>
  b748db:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b748e2:	00 00 
  b748e4:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b748e9:	be 08 00 00 00       	mov    $0x8,%esi
  b748ee:	4c 89 ea             	mov    %r13,%rdx
  b748f1:	ff 15 09 a0 ec 00    	call   *0xeca009(%rip)        # 1a3e900 <posix_memalign@GLIBC_2.2.5>
  b748f7:	85 c0                	test   %eax,%eax
  b748f9:	0f 85 3f 04 00 00    	jne    b74d3e <badger_runtime::gc::alloc+0x76e>
  b748ff:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b74904:	4d 85 e4             	test   %r12,%r12
  b74907:	0f 84 31 04 00 00    	je     b74d3e <badger_runtime::gc::alloc+0x76e>
  b7490d:	4c 89 e7             	mov    %r12,%rdi
  b74910:	31 f6                	xor    %esi,%esi
  b74912:	4c 89 ea             	mov    %r13,%rdx
  b74915:	ff 15 0d a0 ec 00    	call   *0xeca00d(%rip)        # 1a3e928 <memset@GLIBC_2.2.5>
  b7491b:	e9 0b 01 00 00       	jmp    b74a2b <badger_runtime::gc::alloc+0x45b>
  b74920:	49 8b ad 20 01 00 00 	mov    0x120(%r13),%rbp
  b74927:	48 39 d5             	cmp    %rdx,%rbp
  b7492a:	0f 83 d6 00 00 00    	jae    b74a06 <badger_runtime::gc::alloc+0x436>
  b74930:	48 89 54 24 20       	mov    %rdx,0x20(%rsp)
  b74935:	49 83 7d 10 00       	cmpq   $0x0,0x10(%r13)
  b7493a:	bd 00 00 04 00       	mov    $0x40000,%ebp
  b7493f:	b8 00 00 20 00       	mov    $0x200000,%eax
  b74944:	48 0f 45 e8          	cmovne %rax,%rbp
  b74948:	b9 10 00 00 00       	mov    $0x10,%ecx
  b7494d:	48 0f 45 c8          	cmovne %rax,%rcx
  b74951:	48 89 4c 24 48       	mov    %rcx,0x48(%rsp)
  b74956:	4c 89 74 24 50       	mov    %r14,0x50(%rsp)
  b7495b:	0f 84 12 02 00 00    	je     b74b73 <badger_runtime::gc::alloc+0x5a3>
  b74961:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b74968:	00 00 
  b7496a:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b7496f:	be 00 00 20 00       	mov    $0x200000,%esi
  b74974:	ba 00 00 20 00       	mov    $0x200000,%edx
  b74979:	ff 15 81 9f ec 00    	call   *0xec9f81(%rip)        # 1a3e900 <posix_memalign@GLIBC_2.2.5>
  b7497f:	85 c0                	test   %eax,%eax
  b74981:	0f 85 b7 03 00 00    	jne    b74d3e <badger_runtime::gc::alloc+0x76e>
  b74987:	4c 8b 64 24 08       	mov    0x8(%rsp),%r12
  b7498c:	4d 85 e4             	test   %r12,%r12
  b7498f:	0f 84 a9 03 00 00    	je     b74d3e <badger_runtime::gc::alloc+0x76e>
  b74995:	ba 00 00 20 00       	mov    $0x200000,%edx
  b7499a:	4c 89 e7             	mov    %r12,%rdi
  b7499d:	31 f6                	xor    %esi,%esi
  b7499f:	ff 15 83 9f ec 00    	call   *0xec9f83(%rip)        # 1a3e928 <memset@GLIBC_2.2.5>
  b749a5:	be 00 00 20 00       	mov    $0x200000,%esi
  b749aa:	4c 89 e7             	mov    %r12,%rdi
  b749ad:	ba 0e 00 00 00       	mov    $0xe,%edx
  b749b2:	ff 15 10 e1 ec 00    	call   *0xece110(%rip)        # 1a42ac8 <madvise@GLIBC_2.2.5>
  b749b8:	4d 8b 75 10          	mov    0x10(%r13),%r14
  b749bc:	4d 3b 75 00          	cmp    0x0(%r13),%r14
  b749c0:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b749c5:	0f 84 d6 01 00 00    	je     b74ba1 <badger_runtime::gc::alloc+0x5d1>
  b749cb:	49 8b 45 08          	mov    0x8(%r13),%rax
  b749cf:	4b 8d 0c 76          	lea    (%r14,%r14,2),%rcx
  b749d3:	4c 89 24 c8          	mov    %r12,(%rax,%rcx,8)
  b749d7:	48 89 6c c8 08       	mov    %rbp,0x8(%rax,%rcx,8)
  b749dc:	48 8b 74 24 48       	mov    0x48(%rsp),%rsi
  b749e1:	48 89 74 c8 10       	mov    %rsi,0x10(%rax,%rcx,8)
  b749e6:	49 ff c6             	inc    %r14
  b749e9:	4d 89 75 10          	mov    %r14,0x10(%r13)
  b749ed:	4d 89 a5 28 01 00 00 	mov    %r12,0x128(%r13)
  b749f4:	48 8b 05 1d e0 ec 00 	mov    0xece01d(%rip),%rax        # 1a42a18 <_DYNAMIC+0x4328>
  b749fb:	f0 48 01 28          	lock add %rbp,(%rax)
  b749ff:	4c 8b 74 24 50       	mov    0x50(%rsp),%r14
  b74a04:	eb 07                	jmp    b74a0d <badger_runtime::gc::alloc+0x43d>
  b74a06:	4d 8b a5 18 01 00 00 	mov    0x118(%r13),%r12
  b74a0d:	49 8d 04 14          	lea    (%r12,%rdx,1),%rax
  b74a11:	49 89 85 18 01 00 00 	mov    %rax,0x118(%r13)
  b74a18:	48 29 d5             	sub    %rdx,%rbp
  b74a1b:	49 89 ad 20 01 00 00 	mov    %rbp,0x120(%r13)
  b74a22:	4d 85 e4             	test   %r12,%r12
  b74a25:	0f 84 13 03 00 00    	je     b74d3e <badger_runtime::gc::alloc+0x76e>
  b74a2b:	4d 89 34 24          	mov    %r14,(%r12)
  b74a2f:	48 89 d8             	mov    %rbx,%rax
  b74a32:	48 c1 e0 20          	shl    $0x20,%rax
  b74a36:	49 89 44 24 08       	mov    %rax,0x8(%r12)
  b74a3b:	49 83 ff 40          	cmp    $0x40,%r15
  b74a3f:	0f 83 91 02 00 00    	jae    b74cd6 <badger_runtime::gc::alloc+0x706>
  b74a45:	4b 8d 04 7f          	lea    (%r15,%r15,2),%rax
  b74a49:	48 8b 0d a8 d3 ec 00 	mov    0xecd3a8(%rip),%rcx        # 1a41df8 <_DYNAMIC+0x3708>
  b74a50:	4c 8d 34 c1          	lea    (%rcx,%rax,8),%r14
  b74a54:	4c 8b 6c c1 10       	mov    0x10(%rcx,%rax,8),%r13
  b74a59:	4c 3b 2c c1          	cmp    (%rcx,%rax,8),%r13
  b74a5d:	0f 84 02 01 00 00    	je     b74b65 <badger_runtime::gc::alloc+0x595>
  b74a63:	49 8b 46 08          	mov    0x8(%r14),%rax
  b74a67:	4e 89 24 e8          	mov    %r12,(%rax,%r13,8)
  b74a6b:	49 ff c5             	inc    %r13
  b74a6e:	4d 89 6e 10          	mov    %r13,0x10(%r14)
  b74a72:	49 c1 e7 06          	shl    $0x6,%r15
  b74a76:	48 8b 05 53 e0 ec 00 	mov    0xece053(%rip),%rax        # 1a42ad0 <_DYNAMIC+0x43e0>
  b74a7d:	4e 89 2c 38          	mov    %r13,(%rax,%r15,1)
  b74a81:	48 8b 0d 30 e0 ec 00 	mov    0xece030(%rip),%rcx        # 1a42ab8 <_DYNAMIC+0x43c8>
  b74a88:	4a 8d 04 39          	lea    (%rcx,%r15,1),%rax
  b74a8c:	4a 8b 0c 39          	mov    (%rcx,%r15,1),%rcx
  b74a90:	48 01 d9             	add    %rbx,%rcx
  b74a93:	48 83 c1 10          	add    $0x10,%rcx
  b74a97:	48 81 f9 ff ff 00 00 	cmp    $0xffff,%rcx
  b74a9e:	76 14                	jbe    b74ab4 <badger_runtime::gc::alloc+0x4e4>
  b74aa0:	48 c7 00 00 00 00 00 	movq   $0x0,(%rax)
  b74aa7:	48 8b 05 f2 df ec 00 	mov    0xecdff2(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b74aae:	f0 48 01 08          	lock add %rcx,(%rax)
  b74ab2:	eb 03                	jmp    b74ab7 <badger_runtime::gc::alloc+0x4e7>
  b74ab4:	48 89 08             	mov    %rcx,(%rax)
  b74ab7:	4c 89 e0             	mov    %r12,%rax
  b74aba:	48 83 c4 58          	add    $0x58,%rsp
  b74abe:	5b                   	pop    %rbx
  b74abf:	41 5c                	pop    %r12
  b74ac1:	41 5d                	pop    %r13
  b74ac3:	41 5e                	pop    %r14
  b74ac5:	41 5f                	pop    %r15
  b74ac7:	5d                   	pop    %rbp
  b74ac8:	c3                   	ret
  b74ac9:	48 8b 05 30 cc ed 00 	mov    0xedcc30(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b74ad0:	48 83 f8 02          	cmp    $0x2,%rax
  b74ad4:	4d 89 ee             	mov    %r13,%r14
  b74ad7:	72 2f                	jb     b74b08 <badger_runtime::gc::alloc+0x538>
  b74ad9:	ff 15 61 d0 ec 00    	call   *0xecd061(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b74adf:	48 83 f8 40          	cmp    $0x40,%rax
  b74ae3:	0f 83 82 02 00 00    	jae    b74d6b <badger_runtime::gc::alloc+0x79b>
  b74ae9:	48 c1 e0 07          	shl    $0x7,%rax
  b74aed:	48 8d 0d 0c 3a ee 00 	lea    0xee3a0c(%rip),%rcx        # 1a58500 <badger_runtime::task::IN_COMPILED>
  b74af4:	f0 48 ff 0c 08       	lock decq (%rax,%rcx,1)
  b74af9:	e8 42 45 01 00       	call   b89040 <badger_runtime::task::chaos_jitter>
  b74afe:	bf 05 00 00 00       	mov    $0x5,%edi
  b74b03:	e8 a8 ee 01 00       	call   b939b0 <badger_runtime::task::lock_for>
  b74b08:	48 8b 05 91 df ec 00 	mov    0xecdf91(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b74b0f:	48 8b 00             	mov    (%rax),%rax
  b74b12:	49 8b 0f             	mov    (%r15),%rcx
  b74b15:	48 01 c1             	add    %rax,%rcx
  b74b18:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b74b1f:	48 0f 43 c1          	cmovae %rcx,%rax
  b74b23:	49 8b 0c 24          	mov    (%r12),%rcx
  b74b27:	48 39 c8             	cmp    %rcx,%rax
  b74b2a:	0f 87 d2 00 00 00    	ja     b74c02 <badger_runtime::gc::alloc+0x632>
  b74b30:	8b 05 d2 ce ed 00    	mov    0xedced2(%rip),%eax        # 1a51a08 <badger_runtime::gc::stress::STRESS>
  b74b36:	85 c0                	test   %eax,%eax
  b74b38:	0f 85 ae 00 00 00    	jne    b74bec <badger_runtime::gc::alloc+0x61c>
  b74b3e:	40 0a 2d c7 ce ed 00 	or     0xedcec7(%rip),%bpl        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b74b45:	40 80 fd 01          	cmp    $0x1,%bpl
  b74b49:	0f 84 b3 00 00 00    	je     b74c02 <badger_runtime::gc::alloc+0x632>
  b74b4f:	48 8b 05 aa cb ed 00 	mov    0xedcbaa(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b74b56:	48 83 f8 02          	cmp    $0x2,%rax
  b74b5a:	0f 82 b7 fc ff ff    	jb     b74817 <badger_runtime::gc::alloc+0x247>
  b74b60:	e9 42 01 00 00       	jmp    b74ca7 <badger_runtime::gc::alloc+0x6d7>
  b74b65:	4c 89 f7             	mov    %r14,%rdi
  b74b68:	ff 15 ca cf ec 00    	call   *0xeccfca(%rip)        # 1a41b38 <_DYNAMIC+0x3448>
  b74b6e:	e9 f0 fe ff ff       	jmp    b74a63 <badger_runtime::gc::alloc+0x493>
  b74b73:	bf 00 00 04 00       	mov    $0x40000,%edi
  b74b78:	be 01 00 00 00       	mov    $0x1,%esi
  b74b7d:	ff 15 9d 9d ec 00    	call   *0xec9d9d(%rip)        # 1a3e920 <calloc@GLIBC_2.2.5>
  b74b83:	48 85 c0             	test   %rax,%rax
  b74b86:	0f 84 b2 01 00 00    	je     b74d3e <badger_runtime::gc::alloc+0x76e>
  b74b8c:	49 89 c4             	mov    %rax,%r12
  b74b8f:	45 31 f6             	xor    %r14d,%r14d
  b74b92:	4d 3b 75 00          	cmp    0x0(%r13),%r14
  b74b96:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b74b9b:	0f 85 2a fe ff ff    	jne    b749cb <badger_runtime::gc::alloc+0x3fb>
  b74ba1:	4c 89 ef             	mov    %r13,%rdi
  b74ba4:	ff 15 ce d1 ec 00    	call   *0xecd1ce(%rip)        # 1a41d78 <_DYNAMIC+0x3688>
  b74baa:	48 8b 54 24 20       	mov    0x20(%rsp),%rdx
  b74baf:	e9 17 fe ff ff       	jmp    b749cb <badger_runtime::gc::alloc+0x3fb>
  b74bb4:	e8 19 13 f5 ff       	call   ac5ed2 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b74bb9:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b74bc0:	ff ff 
  b74bc2:	48 85 c0             	test   %rax,%rax
  b74bc5:	0f 85 8a fa ff ff    	jne    b74655 <badger_runtime::gc::alloc+0x85>
  b74bcb:	e9 4c fa ff ff       	jmp    b7461c <badger_runtime::gc::alloc+0x4c>
  b74bd0:	e8 3a 1c f5 ff       	call   ac680f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b74bd5:	0f b6 05 30 ce ed 00 	movzbl 0xedce30(%rip),%eax        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b74bdc:	40 08 e8             	or     %bpl,%al
  b74bdf:	3c 01                	cmp    $0x1,%al
  b74be1:	0f 84 2f fb ff ff    	je     b74716 <badger_runtime::gc::alloc+0x146>
  b74be7:	e9 2b fc ff ff       	jmp    b74817 <badger_runtime::gc::alloc+0x247>
  b74bec:	e8 1e 1c f5 ff       	call   ac680f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b74bf1:	40 0a 2d 14 ce ed 00 	or     0xedce14(%rip),%bpl        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b74bf8:	40 80 fd 01          	cmp    $0x1,%bpl
  b74bfc:	0f 85 4d ff ff ff    	jne    b74b4f <badger_runtime::gc::alloc+0x57f>
  b74c02:	bf 01 00 00 00       	mov    $0x1,%edi
  b74c07:	e8 44 62 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b74c0c:	49 89 c7             	mov    %rax,%r15
  b74c0f:	89 d5                	mov    %edx,%ebp
  b74c11:	ff 15 a9 de ec 00    	call   *0xecdea9(%rip)        # 1a42ac0 <_DYNAMIC+0x43d0>
  b74c17:	e8 f4 a5 ff ff       	call   b6f210 <badger_runtime::gc::collect_inner>
  b74c1c:	45 31 f6             	xor    %r14d,%r14d
  b74c1f:	31 c0                	xor    %eax,%eax
  b74c21:	86 05 e9 58 ee 00    	xchg   %al,0xee58e9(%rip)        # 1a5a510 <badger_runtime::task::STOP>
  b74c27:	bf 01 00 00 00       	mov    $0x1,%edi
  b74c2c:	e8 1f 62 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b74c31:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b74c36:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b74c3a:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b74c3f:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b74c43:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b74c48:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b74c4d:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b74c52:	e8 29 61 a7 00       	call   15ead80 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b74c57:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b74c5b:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b74c62:	9a 3b 
  b74c64:	48 01 c1             	add    %rax,%rcx
  b74c67:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b74c6c:	4c 0f 44 f1          	cmove  %rcx,%r14
  b74c70:	48 8b 05 b1 ac ec 00 	mov    0xecacb1(%rip),%rax        # 1a3f928 <_DYNAMIC+0x1238>
  b74c77:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b74c7e:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b74c85:	49 39 ce             	cmp    %rcx,%r14
  b74c88:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b74c8c:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b74c93:	4d 89 ee             	mov    %r13,%r14
  b74c96:	48 8b 05 63 ca ed 00 	mov    0xedca63(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b74c9d:	48 83 f8 02          	cmp    $0x2,%rax
  b74ca1:	0f 82 70 fb ff ff    	jb     b74817 <badger_runtime::gc::alloc+0x247>
  b74ca7:	ff 15 93 ce ec 00    	call   *0xecce93(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b74cad:	48 83 f8 40          	cmp    $0x40,%rax
  b74cb1:	0f 83 cb 00 00 00    	jae    b74d82 <badger_runtime::gc::alloc+0x7b2>
  b74cb7:	48 c1 e0 07          	shl    $0x7,%rax
  b74cbb:	48 8d 0d 3e 38 ee 00 	lea    0xee383e(%rip),%rcx        # 1a58500 <badger_runtime::task::IN_COMPILED>
  b74cc2:	f0 48 ff 04 08       	lock incq (%rax,%rcx,1)
  b74cc7:	e8 f4 d6 01 00       	call   b923c0 <badger_runtime::task::unlock>
  b74ccc:	e8 6f 43 01 00       	call   b89040 <badger_runtime::task::chaos_jitter>
  b74cd1:	e9 41 fb ff ff       	jmp    b74817 <badger_runtime::gc::alloc+0x247>
  b74cd6:	48 8d 15 6b d9 e4 00 	lea    0xe4d96b(%rip),%rdx        # 19c2648 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0xf20>
  b74cdd:	be 40 00 00 00       	mov    $0x40,%esi
  b74ce2:	4c 89 ff             	mov    %r15,%rdi
  b74ce5:	ff 15 65 9c ec 00    	call   *0xec9c65(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74ceb:	48 8d 3d f9 8d 6f ff 	lea    -0x907207(%rip),%rdi        # 26daeb <anon.5680abfc9ce5520a59545b4df02fd4ad.533.llvm.1751364238661663638>
  b74cf2:	48 8d 0d 7f e4 e4 00 	lea    0xe4e47f(%rip),%rcx        # 19c3178 <anon.5680abfc9ce5520a59545b4df02fd4ad.563.llvm.1751364238661663638+0x160>
  b74cf9:	4c 8d 05 30 f4 e4 00 	lea    0xe4f430(%rip),%r8        # 19c4130 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x1a0>
  b74d00:	48 8d 54 24 07       	lea    0x7(%rsp),%rdx
  b74d05:	be 2b 00 00 00       	mov    $0x2b,%esi
  b74d0a:	ff 15 80 9d ec 00    	call   *0xec9d80(%rip)        # 1a3ea90 <_DYNAMIC+0x3a0>
  b74d10:	48 8d 15 c1 cb e4 00 	lea    0xe4cbc1(%rip),%rdx        # 19c18d8 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0x1b0>
  b74d17:	be 40 00 00 00       	mov    $0x40,%esi
  b74d1c:	4c 89 ff             	mov    %r15,%rdi
  b74d1f:	ff 15 2b 9c ec 00    	call   *0xec9c2b(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74d25:	48 8d 15 c4 cb e4 00 	lea    0xe4cbc4(%rip),%rdx        # 19c18f0 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0x1c8>
  b74d2c:	be 20 00 00 00       	mov    $0x20,%esi
  b74d31:	48 c7 c7 ff ff ff ff 	mov    $0xffffffffffffffff,%rdi
  b74d38:	ff 15 12 9c ec 00    	call   *0xec9c12(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74d3e:	48 8d 3d a7 87 6f ff 	lea    -0x907859(%rip),%rdi        # 26d4ec <anon.3a879afe13029b9ad6f5d17cd43d38ab.43.llvm.1747630503651334518+0x135aa>
  b74d45:	be 2b 00 00 00       	mov    $0x2b,%esi
  b74d4a:	ff 15 b0 a3 ec 00    	call   *0xeca3b0(%rip)        # 1a3f100 <_DYNAMIC+0xa10>
  b74d50:	ff 15 4a a4 ec 00    	call   *0xeca44a(%rip)        # 1a3f1a0 <_DYNAMIC+0xab0>
  b74d56:	48 8d 15 93 f2 e4 00 	lea    0xe4f293(%rip),%rdx        # 19c3ff0 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x60>
  b74d5d:	be 40 00 00 00       	mov    $0x40,%esi
  b74d62:	48 89 c7             	mov    %rax,%rdi
  b74d65:	ff 15 e5 9b ec 00    	call   *0xec9be5(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74d6b:	48 8d 15 0e f7 e4 00 	lea    0xe4f70e(%rip),%rdx        # 19c4480 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x4f0>
  b74d72:	be 40 00 00 00       	mov    $0x40,%esi
  b74d77:	48 89 c7             	mov    %rax,%rdi
  b74d7a:	ff 15 d0 9b ec 00    	call   *0xec9bd0(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74d80:	eb 15                	jmp    b74d97 <badger_runtime::gc::alloc+0x7c7>
  b74d82:	48 8d 15 f7 f6 e4 00 	lea    0xe4f6f7(%rip),%rdx        # 19c4480 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x4f0>
  b74d89:	be 40 00 00 00       	mov    $0x40,%esi
  b74d8e:	48 89 c7             	mov    %rax,%rdi
  b74d91:	ff 15 b9 9b ec 00    	call   *0xec9bb9(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b74d97:	0f 0b                	ud2
  b74d99:	ff 15 b9 a2 ec 00    	call   *0xeca2b9(%rip)        # 1a3f058 <_DYNAMIC+0x968>
  b74d9f:	ff 15 b3 a2 ec 00    	call   *0xeca2b3(%rip)        # 1a3f058 <_DYNAMIC+0x968>
