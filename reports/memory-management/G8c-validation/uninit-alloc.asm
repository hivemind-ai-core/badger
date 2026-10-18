
/home/dan/orca/projects/language/target/memory-binaries-2026-10-18/hamt-init-candidate:     file format elf64-x86-64


Disassembly of section .text:

0000000000b6e3b0 <badger_runtime::gc::alloc_uninit>:
  b6e3b0:	55                   	push   %rbp
  b6e3b1:	41 57                	push   %r15
  b6e3b3:	41 56                	push   %r14
  b6e3b5:	41 55                	push   %r13
  b6e3b7:	41 54                	push   %r12
  b6e3b9:	53                   	push   %rbx
  b6e3ba:	48 83 ec 58          	sub    $0x58,%rsp
  b6e3be:	48 89 f3             	mov    %rsi,%rbx
  b6e3c1:	49 89 fd             	mov    %rdi,%r13
  b6e3c4:	8b 05 d6 36 ee 00    	mov    0xee36d6(%rip),%eax        # 1a51aa0 <badger_runtime::task::chaos::CHAOS+0x10>
  b6e3ca:	85 c0                	test   %eax,%eax
  b6e3cc:	0f 85 b7 00 00 00    	jne    b6e489 <badger_runtime::gc::alloc_uninit+0xd9>
  b6e3d2:	48 83 3d b6 36 ee 00 	cmpq   $0x0,0xee36b6(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b6e3d9:	00 
  b6e3da:	0f 84 bc 00 00 00    	je     b6e49c <badger_runtime::gc::alloc_uninit+0xec>
  b6e3e0:	8b 05 ba 36 ee 00    	mov    0xee36ba(%rip),%eax        # 1a51aa0 <badger_runtime::task::chaos::CHAOS+0x10>
  b6e3e6:	85 c0                	test   %eax,%eax
  b6e3e8:	0f 85 77 05 00 00    	jne    b6e965 <badger_runtime::gc::alloc_uninit+0x5b5>
  b6e3ee:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b6e3f5:	ff ff 
  b6e3f7:	48 85 c0             	test   %rax,%rax
  b6e3fa:	75 33                	jne    b6e42f <badger_runtime::gc::alloc_uninit+0x7f>
  b6e3fc:	80 3d 8d 36 ee 00 00 	cmpb   $0x0,0xee368d(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b6e403:	74 09                	je     b6e40e <badger_runtime::gc::alloc_uninit+0x5e>
  b6e405:	4c 8b 35 8c 36 ee 00 	mov    0xee368c(%rip),%r14        # 1a51a98 <badger_runtime::task::chaos::CHAOS+0x8>
  b6e40c:	eb 03                	jmp    b6e411 <badger_runtime::gc::alloc_uninit+0x61>
  b6e40e:	45 31 f6             	xor    %r14d,%r14d
  b6e411:	ff 15 29 37 ed 00    	call   *0xed3729(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6e417:	48 b9 03 ed 92 d1 32 	movabs $0xd1b54a32d192ed03,%rcx
  b6e41e:	4a b5 d1 
  b6e421:	48 0f af c1          	imul   %rcx,%rax
  b6e425:	48 01 c8             	add    %rcx,%rax
  b6e428:	4c 31 f0             	xor    %r14,%rax
  b6e42b:	48 83 c8 01          	or     $0x1,%rax
  b6e42f:	48 b9 15 7c 4a 7f b9 	movabs $0x9e3779b97f4a7c15,%rcx
  b6e436:	79 37 9e 
  b6e439:	48 01 c1             	add    %rax,%rcx
  b6e43c:	64 48 89 0c 25 28 fb 	mov    %rcx,%fs:0xfffffffffffffb28
  b6e443:	ff ff 
  b6e445:	48 89 c8             	mov    %rcx,%rax
  b6e448:	48 c1 e8 1e          	shr    $0x1e,%rax
  b6e44c:	48 31 c8             	xor    %rcx,%rax
  b6e44f:	48 b9 b9 e5 e4 1c 6d 	movabs $0xbf58476d1ce4e5b9,%rcx
  b6e456:	47 58 bf 
  b6e459:	48 0f af c8          	imul   %rax,%rcx
  b6e45d:	48 89 c8             	mov    %rcx,%rax
  b6e460:	48 c1 e8 1b          	shr    $0x1b,%rax
  b6e464:	48 31 c8             	xor    %rcx,%rax
  b6e467:	48 b9 eb 11 31 13 bb 	movabs $0x94d049bb133111eb,%rcx
  b6e46e:	49 d0 94 
  b6e471:	48 0f af c8          	imul   %rax,%rcx
  b6e475:	48 89 c8             	mov    %rcx,%rax
  b6e478:	48 c1 e8 1f          	shr    $0x1f,%rax
  b6e47c:	31 c8                	xor    %ecx,%eax
  b6e47e:	a9 ff 01 00 00       	test   $0x1ff,%eax
  b6e483:	40 0f 94 c5          	sete   %bpl
  b6e487:	eb 15                	jmp    b6e49e <badger_runtime::gc::alloc_uninit+0xee>
  b6e489:	e8 44 7a f5 ff       	call   ac5ed2 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b6e48e:	48 83 3d fa 35 ee 00 	cmpq   $0x0,0xee35fa(%rip)        # 1a51a90 <badger_runtime::task::chaos::CHAOS>
  b6e495:	00 
  b6e496:	0f 85 44 ff ff ff    	jne    b6e3e0 <badger_runtime::gc::alloc_uninit+0x30>
  b6e49c:	31 ed                	xor    %ebp,%ebp
  b6e49e:	48 8b 05 fb 45 ed 00 	mov    0xed45fb(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b6e4a5:	48 8b 00             	mov    (%rax),%rax
  b6e4a8:	4c 8b 3d 91 3a ed 00 	mov    0xed3a91(%rip),%r15        # 1a41f40 <_DYNAMIC+0x3850>
  b6e4af:	49 8b 0f             	mov    (%r15),%rcx
  b6e4b2:	48 01 c1             	add    %rax,%rcx
  b6e4b5:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b6e4bc:	48 0f 43 c1          	cmovae %rcx,%rax
  b6e4c0:	4c 8b 35 e1 45 ed 00 	mov    0xed45e1(%rip),%r14        # 1a42aa8 <_DYNAMIC+0x43b8>
  b6e4c7:	49 8b 0e             	mov    (%r14),%rcx
  b6e4ca:	48 39 c8             	cmp    %rcx,%rax
  b6e4cd:	77 20                	ja     b6e4ef <badger_runtime::gc::alloc_uninit+0x13f>
  b6e4cf:	8b 05 33 35 ee 00    	mov    0xee3533(%rip),%eax        # 1a51a08 <badger_runtime::gc::stress::STRESS>
  b6e4d5:	85 c0                	test   %eax,%eax
  b6e4d7:	0f 85 a4 04 00 00    	jne    b6e981 <badger_runtime::gc::alloc_uninit+0x5d1>
  b6e4dd:	0f b6 05 28 35 ee 00 	movzbl 0xee3528(%rip),%eax        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b6e4e4:	40 08 e8             	or     %bpl,%al
  b6e4e7:	3c 01                	cmp    $0x1,%al
  b6e4e9:	0f 85 fb 00 00 00    	jne    b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6e4ef:	48 8b 05 ba 45 ed 00 	mov    0xed45ba(%rip),%rax        # 1a42ab0 <_DYNAMIC+0x43c0>
  b6e4f6:	48 8b 00             	mov    (%rax),%rax
  b6e4f9:	48 85 c0             	test   %rax,%rax
  b6e4fc:	0f 85 e8 00 00 00    	jne    b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6e502:	ff 15 38 36 ed 00    	call   *0xed3638(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6e508:	48 83 f8 3f          	cmp    $0x3f,%rax
  b6e50c:	0f 87 f2 05 00 00    	ja     b6eb04 <badger_runtime::gc::alloc_uninit+0x754>
  b6e512:	48 c1 e0 06          	shl    $0x6,%rax
  b6e516:	48 8b 15 9b 45 ed 00 	mov    0xed459b(%rip),%rdx        # 1a42ab8 <_DYNAMIC+0x43c8>
  b6e51d:	31 c9                	xor    %ecx,%ecx
  b6e51f:	48 87 0c 02          	xchg   %rcx,(%rdx,%rax,1)
  b6e523:	48 85 c9             	test   %rcx,%rcx
  b6e526:	74 0b                	je     b6e533 <badger_runtime::gc::alloc_uninit+0x183>
  b6e528:	48 8b 05 71 45 ed 00 	mov    0xed4571(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b6e52f:	f0 48 01 08          	lock add %rcx,(%rax)
  b6e533:	48 8b 05 c6 31 ee 00 	mov    0xee31c6(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b6e53a:	48 83 f8 02          	cmp    $0x2,%rax
  b6e53e:	72 19                	jb     b6e559 <badger_runtime::gc::alloc_uninit+0x1a9>
  b6e540:	4c 8b 25 d1 bf ee 00 	mov    0xeebfd1(%rip),%r12        # 1a5a518 <badger_runtime::task::OWNER>
  b6e547:	ff 15 f3 35 ed 00    	call   *0xed35f3(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6e54d:	48 ff c0             	inc    %rax
  b6e550:	49 39 c4             	cmp    %rax,%r12
  b6e553:	0f 85 27 03 00 00    	jne    b6e880 <badger_runtime::gc::alloc_uninit+0x4d0>
  b6e559:	bf 01 00 00 00       	mov    $0x1,%edi
  b6e55e:	e8 ed c8 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b6e563:	49 89 c7             	mov    %rax,%r15
  b6e566:	89 d5                	mov    %edx,%ebp
  b6e568:	ff 15 52 45 ed 00    	call   *0xed4552(%rip)        # 1a42ac0 <_DYNAMIC+0x43d0>
  b6e56e:	e8 9d 0c 00 00       	call   b6f210 <badger_runtime::gc::collect_inner>
  b6e573:	45 31 f6             	xor    %r14d,%r14d
  b6e576:	31 c0                	xor    %eax,%eax
  b6e578:	86 05 92 bf ee 00    	xchg   %al,0xeebf92(%rip)        # 1a5a510 <badger_runtime::task::STOP>
  b6e57e:	bf 01 00 00 00       	mov    $0x1,%edi
  b6e583:	e8 c8 c8 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b6e588:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b6e58d:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b6e591:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b6e596:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b6e59a:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b6e59f:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b6e5a4:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b6e5a9:	e8 d2 c7 a7 00       	call   15ead80 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b6e5ae:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b6e5b2:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b6e5b9:	9a 3b 
  b6e5bb:	48 01 c1             	add    %rax,%rcx
  b6e5be:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b6e5c3:	4c 0f 44 f1          	cmove  %rcx,%r14
  b6e5c7:	48 8b 05 5a 13 ed 00 	mov    0xed135a(%rip),%rax        # 1a3f928 <_DYNAMIC+0x1238>
  b6e5ce:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b6e5d5:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b6e5dc:	49 39 ce             	cmp    %rcx,%r14
  b6e5df:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b6e5e3:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b6e5ea:	ff 15 50 35 ed 00    	call   *0xed3550(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6e5f0:	49 89 c7             	mov    %rax,%r15
  b6e5f3:	48 83 fb 09          	cmp    $0x9,%rbx
  b6e5f7:	ba 08 00 00 00       	mov    $0x8,%edx
  b6e5fc:	48 0f 43 d3          	cmovae %rbx,%rdx
  b6e600:	48 8d 4a 1f          	lea    0x1f(%rdx),%rcx
  b6e604:	48 89 cd             	mov    %rcx,%rbp
  b6e607:	48 83 e5 f0          	and    $0xfffffffffffffff0,%rbp
  b6e60b:	48 81 fd 00 02 00 00 	cmp    $0x200,%rbp
  b6e612:	76 34                	jbe    b6e648 <badger_runtime::gc::alloc_uninit+0x298>
  b6e614:	48 83 c2 10          	add    $0x10,%rdx
  b6e618:	48 b8 f9 ff ff ff ff 	movabs $0x7ffffffffffffff9,%rax
  b6e61f:	ff ff 7f 
  b6e622:	48 39 c2             	cmp    %rax,%rdx
  b6e625:	0f 83 6e 04 00 00    	jae    b6ea99 <badger_runtime::gc::alloc_uninit+0x6e9>
  b6e62b:	48 83 fa 07          	cmp    $0x7,%rdx
  b6e62f:	76 66                	jbe    b6e697 <badger_runtime::gc::alloc_uninit+0x2e7>
  b6e631:	48 89 d7             	mov    %rdx,%rdi
  b6e634:	ff 15 be 02 ed 00    	call   *0xed02be(%rip)        # 1a3e8f8 <malloc@GLIBC_2.2.5>
  b6e63a:	48 85 c0             	test   %rax,%rax
  b6e63d:	0f 85 a4 01 00 00    	jne    b6e7e7 <badger_runtime::gc::alloc_uninit+0x437>
  b6e643:	e9 a4 04 00 00       	jmp    b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e648:	49 83 ff 40          	cmp    $0x40,%r15
  b6e64c:	0f 83 6c 04 00 00    	jae    b6eabe <badger_runtime::gc::alloc_uninit+0x70e>
  b6e652:	48 c1 e9 04          	shr    $0x4,%rcx
  b6e656:	48 8d 41 ff          	lea    -0x1(%rcx),%rax
  b6e65a:	48 83 f8 20          	cmp    $0x20,%rax
  b6e65e:	0f 83 6f 04 00 00    	jae    b6ead3 <badger_runtime::gc::alloc_uninit+0x723>
  b6e664:	4d 69 e7 30 01 00 00 	imul   $0x130,%r15,%r12
  b6e66b:	4c 03 25 8e 37 ed 00 	add    0xed378e(%rip),%r12        # 1a41e00 <_DYNAMIC+0x3710>
  b6e672:	49 8b 44 cc 10       	mov    0x10(%r12,%rcx,8),%rax
  b6e677:	48 85 c0             	test   %rax,%rax
  b6e67a:	74 4f                	je     b6e6cb <badger_runtime::gc::alloc_uninit+0x31b>
  b6e67c:	48 8b 10             	mov    (%rax),%rdx
  b6e67f:	49 89 54 cc 10       	mov    %rdx,0x10(%r12,%rcx,8)
  b6e684:	4c 89 28             	mov    %r13,(%rax)
  b6e687:	48 89 d9             	mov    %rbx,%rcx
  b6e68a:	48 c1 e1 20          	shl    $0x20,%rcx
  b6e68e:	48 89 48 08          	mov    %rcx,0x8(%rax)
  b6e692:	e9 68 01 00 00       	jmp    b6e7ff <badger_runtime::gc::alloc_uninit+0x44f>
  b6e697:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b6e69e:	00 00 
  b6e6a0:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b6e6a5:	be 08 00 00 00       	mov    $0x8,%esi
  b6e6aa:	ff 15 50 02 ed 00    	call   *0xed0250(%rip)        # 1a3e900 <posix_memalign@GLIBC_2.2.5>
  b6e6b0:	85 c0                	test   %eax,%eax
  b6e6b2:	0f 85 34 04 00 00    	jne    b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e6b8:	48 8b 44 24 08       	mov    0x8(%rsp),%rax
  b6e6bd:	48 85 c0             	test   %rax,%rax
  b6e6c0:	0f 85 21 01 00 00    	jne    b6e7e7 <badger_runtime::gc::alloc_uninit+0x437>
  b6e6c6:	e9 21 04 00 00       	jmp    b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e6cb:	4d 8b b4 24 20 01 00 	mov    0x120(%r12),%r14
  b6e6d2:	00 
  b6e6d3:	49 39 ee             	cmp    %rbp,%r14
  b6e6d6:	0f 83 e3 00 00 00    	jae    b6e7bf <badger_runtime::gc::alloc_uninit+0x40f>
  b6e6dc:	49 83 7c 24 10 00    	cmpq   $0x0,0x10(%r12)
  b6e6e2:	41 be 00 00 04 00    	mov    $0x40000,%r14d
  b6e6e8:	b8 00 00 20 00       	mov    $0x200000,%eax
  b6e6ed:	4c 0f 45 f0          	cmovne %rax,%r14
  b6e6f1:	b9 10 00 00 00       	mov    $0x10,%ecx
  b6e6f6:	48 0f 45 c8          	cmovne %rax,%rcx
  b6e6fa:	48 89 4c 24 48       	mov    %rcx,0x48(%rsp)
  b6e6ff:	4c 89 6c 24 50       	mov    %r13,0x50(%rsp)
  b6e704:	0f 84 22 02 00 00    	je     b6e92c <badger_runtime::gc::alloc_uninit+0x57c>
  b6e70a:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  b6e711:	00 00 
  b6e713:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b6e718:	be 00 00 20 00       	mov    $0x200000,%esi
  b6e71d:	ba 00 00 20 00       	mov    $0x200000,%edx
  b6e722:	ff 15 d8 01 ed 00    	call   *0xed01d8(%rip)        # 1a3e900 <posix_memalign@GLIBC_2.2.5>
  b6e728:	85 c0                	test   %eax,%eax
  b6e72a:	0f 85 bc 03 00 00    	jne    b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e730:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  b6e735:	48 85 ff             	test   %rdi,%rdi
  b6e738:	0f 84 ae 03 00 00    	je     b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e73e:	ba 00 00 20 00       	mov    $0x200000,%edx
  b6e743:	31 f6                	xor    %esi,%esi
  b6e745:	49 89 fd             	mov    %rdi,%r13
  b6e748:	ff 15 da 01 ed 00    	call   *0xed01da(%rip)        # 1a3e928 <memset@GLIBC_2.2.5>
  b6e74e:	be 00 00 20 00       	mov    $0x200000,%esi
  b6e753:	4c 89 6c 24 20       	mov    %r13,0x20(%rsp)
  b6e758:	4c 89 ef             	mov    %r13,%rdi
  b6e75b:	ba 0e 00 00 00       	mov    $0xe,%edx
  b6e760:	ff 15 62 43 ed 00    	call   *0xed4362(%rip)        # 1a42ac8 <madvise@GLIBC_2.2.5>
  b6e766:	4d 8b 6c 24 10       	mov    0x10(%r12),%r13
  b6e76b:	4d 3b 2c 24          	cmp    (%r12),%r13
  b6e76f:	0f 84 e2 01 00 00    	je     b6e957 <badger_runtime::gc::alloc_uninit+0x5a7>
  b6e775:	49 8b 4c 24 08       	mov    0x8(%r12),%rcx
  b6e77a:	4a 8d 14 6d 00 00 00 	lea    0x0(,%r13,2),%rdx
  b6e781:	00 
  b6e782:	4c 01 ea             	add    %r13,%rdx
  b6e785:	48 8b 44 24 20       	mov    0x20(%rsp),%rax
  b6e78a:	48 89 04 d1          	mov    %rax,(%rcx,%rdx,8)
  b6e78e:	4c 89 74 d1 08       	mov    %r14,0x8(%rcx,%rdx,8)
  b6e793:	48 8b 74 24 48       	mov    0x48(%rsp),%rsi
  b6e798:	48 89 74 d1 10       	mov    %rsi,0x10(%rcx,%rdx,8)
  b6e79d:	49 ff c5             	inc    %r13
  b6e7a0:	4d 89 6c 24 10       	mov    %r13,0x10(%r12)
  b6e7a5:	49 89 84 24 28 01 00 	mov    %rax,0x128(%r12)
  b6e7ac:	00 
  b6e7ad:	48 8b 0d 64 42 ed 00 	mov    0xed4264(%rip),%rcx        # 1a42a18 <_DYNAMIC+0x4328>
  b6e7b4:	f0 4c 01 31          	lock add %r14,(%rcx)
  b6e7b8:	4c 8b 6c 24 50       	mov    0x50(%rsp),%r13
  b6e7bd:	eb 08                	jmp    b6e7c7 <badger_runtime::gc::alloc_uninit+0x417>
  b6e7bf:	49 8b 84 24 18 01 00 	mov    0x118(%r12),%rax
  b6e7c6:	00 
  b6e7c7:	48 8d 0c 28          	lea    (%rax,%rbp,1),%rcx
  b6e7cb:	49 89 8c 24 18 01 00 	mov    %rcx,0x118(%r12)
  b6e7d2:	00 
  b6e7d3:	49 29 ee             	sub    %rbp,%r14
  b6e7d6:	4d 89 b4 24 20 01 00 	mov    %r14,0x120(%r12)
  b6e7dd:	00 
  b6e7de:	48 85 c0             	test   %rax,%rax
  b6e7e1:	0f 84 05 03 00 00    	je     b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e7e7:	4c 89 28             	mov    %r13,(%rax)
  b6e7ea:	48 89 d9             	mov    %rbx,%rcx
  b6e7ed:	48 c1 e1 20          	shl    $0x20,%rcx
  b6e7f1:	48 89 48 08          	mov    %rcx,0x8(%rax)
  b6e7f5:	49 83 ff 40          	cmp    $0x40,%r15
  b6e7f9:	0f 83 85 02 00 00    	jae    b6ea84 <badger_runtime::gc::alloc_uninit+0x6d4>
  b6e7ff:	4b 8d 0c 7f          	lea    (%r15,%r15,2),%rcx
  b6e803:	48 8b 15 ee 35 ed 00 	mov    0xed35ee(%rip),%rdx        # 1a41df8 <_DYNAMIC+0x3708>
  b6e80a:	4c 8d 34 ca          	lea    (%rdx,%rcx,8),%r14
  b6e80e:	4c 8b 64 ca 10       	mov    0x10(%rdx,%rcx,8),%r12
  b6e813:	4c 3b 24 ca          	cmp    (%rdx,%rcx,8),%r12
  b6e817:	0f 84 fb 00 00 00    	je     b6e918 <badger_runtime::gc::alloc_uninit+0x568>
  b6e81d:	49 8b 4e 08          	mov    0x8(%r14),%rcx
  b6e821:	4a 89 04 e1          	mov    %rax,(%rcx,%r12,8)
  b6e825:	49 ff c4             	inc    %r12
  b6e828:	4d 89 66 10          	mov    %r12,0x10(%r14)
  b6e82c:	49 c1 e7 06          	shl    $0x6,%r15
  b6e830:	48 8b 0d 99 42 ed 00 	mov    0xed4299(%rip),%rcx        # 1a42ad0 <_DYNAMIC+0x43e0>
  b6e837:	4e 89 24 39          	mov    %r12,(%rcx,%r15,1)
  b6e83b:	48 8b 15 76 42 ed 00 	mov    0xed4276(%rip),%rdx        # 1a42ab8 <_DYNAMIC+0x43c8>
  b6e842:	4a 8d 0c 3a          	lea    (%rdx,%r15,1),%rcx
  b6e846:	4a 8b 14 3a          	mov    (%rdx,%r15,1),%rdx
  b6e84a:	48 01 da             	add    %rbx,%rdx
  b6e84d:	48 83 c2 10          	add    $0x10,%rdx
  b6e851:	48 81 fa ff ff 00 00 	cmp    $0xffff,%rdx
  b6e858:	76 14                	jbe    b6e86e <badger_runtime::gc::alloc_uninit+0x4be>
  b6e85a:	48 c7 01 00 00 00 00 	movq   $0x0,(%rcx)
  b6e861:	48 8b 0d 38 42 ed 00 	mov    0xed4238(%rip),%rcx        # 1a42aa0 <_DYNAMIC+0x43b0>
  b6e868:	f0 48 01 11          	lock add %rdx,(%rcx)
  b6e86c:	eb 03                	jmp    b6e871 <badger_runtime::gc::alloc_uninit+0x4c1>
  b6e86e:	48 89 11             	mov    %rdx,(%rcx)
  b6e871:	48 83 c4 58          	add    $0x58,%rsp
  b6e875:	5b                   	pop    %rbx
  b6e876:	41 5c                	pop    %r12
  b6e878:	41 5d                	pop    %r13
  b6e87a:	41 5e                	pop    %r14
  b6e87c:	41 5f                	pop    %r15
  b6e87e:	5d                   	pop    %rbp
  b6e87f:	c3                   	ret
  b6e880:	48 8b 05 79 2e ee 00 	mov    0xee2e79(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b6e887:	48 83 f8 02          	cmp    $0x2,%rax
  b6e88b:	72 2f                	jb     b6e8bc <badger_runtime::gc::alloc_uninit+0x50c>
  b6e88d:	ff 15 ad 32 ed 00    	call   *0xed32ad(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6e893:	48 83 f8 40          	cmp    $0x40,%rax
  b6e897:	0f 83 7c 02 00 00    	jae    b6eb19 <badger_runtime::gc::alloc_uninit+0x769>
  b6e89d:	48 c1 e0 07          	shl    $0x7,%rax
  b6e8a1:	48 8d 0d 58 9c ee 00 	lea    0xee9c58(%rip),%rcx        # 1a58500 <badger_runtime::task::IN_COMPILED>
  b6e8a8:	f0 48 ff 0c 08       	lock decq (%rax,%rcx,1)
  b6e8ad:	e8 8e a7 01 00       	call   b89040 <badger_runtime::task::chaos_jitter>
  b6e8b2:	bf 05 00 00 00       	mov    $0x5,%edi
  b6e8b7:	e8 f4 50 02 00       	call   b939b0 <badger_runtime::task::lock_for>
  b6e8bc:	48 8b 05 dd 41 ed 00 	mov    0xed41dd(%rip),%rax        # 1a42aa0 <_DYNAMIC+0x43b0>
  b6e8c3:	48 8b 00             	mov    (%rax),%rax
  b6e8c6:	49 8b 0f             	mov    (%r15),%rcx
  b6e8c9:	48 01 c1             	add    %rax,%rcx
  b6e8cc:	48 c7 c0 ff ff ff ff 	mov    $0xffffffffffffffff,%rax
  b6e8d3:	48 0f 43 c1          	cmovae %rcx,%rax
  b6e8d7:	49 8b 0e             	mov    (%r14),%rcx
  b6e8da:	48 39 c8             	cmp    %rcx,%rax
  b6e8dd:	0f 87 d0 00 00 00    	ja     b6e9b3 <badger_runtime::gc::alloc_uninit+0x603>
  b6e8e3:	8b 05 1f 31 ee 00    	mov    0xee311f(%rip),%eax        # 1a51a08 <badger_runtime::gc::stress::STRESS>
  b6e8e9:	85 c0                	test   %eax,%eax
  b6e8eb:	0f 85 ac 00 00 00    	jne    b6e99d <badger_runtime::gc::alloc_uninit+0x5ed>
  b6e8f1:	40 0a 2d 14 31 ee 00 	or     0xee3114(%rip),%bpl        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b6e8f8:	40 80 fd 01          	cmp    $0x1,%bpl
  b6e8fc:	0f 84 b1 00 00 00    	je     b6e9b3 <badger_runtime::gc::alloc_uninit+0x603>
  b6e902:	48 8b 05 f7 2d ee 00 	mov    0xee2df7(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b6e909:	48 83 f8 02          	cmp    $0x2,%rax
  b6e90d:	0f 82 d7 fc ff ff    	jb     b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6e913:	e9 3d 01 00 00       	jmp    b6ea55 <badger_runtime::gc::alloc_uninit+0x6a5>
  b6e918:	4c 89 f7             	mov    %r14,%rdi
  b6e91b:	49 89 c5             	mov    %rax,%r13
  b6e91e:	ff 15 14 32 ed 00    	call   *0xed3214(%rip)        # 1a41b38 <_DYNAMIC+0x3448>
  b6e924:	4c 89 e8             	mov    %r13,%rax
  b6e927:	e9 f1 fe ff ff       	jmp    b6e81d <badger_runtime::gc::alloc_uninit+0x46d>
  b6e92c:	bf 00 00 04 00       	mov    $0x40000,%edi
  b6e931:	be 01 00 00 00       	mov    $0x1,%esi
  b6e936:	ff 15 e4 ff ec 00    	call   *0xecffe4(%rip)        # 1a3e920 <calloc@GLIBC_2.2.5>
  b6e93c:	48 85 c0             	test   %rax,%rax
  b6e93f:	0f 84 a7 01 00 00    	je     b6eaec <badger_runtime::gc::alloc_uninit+0x73c>
  b6e945:	48 89 44 24 20       	mov    %rax,0x20(%rsp)
  b6e94a:	45 31 ed             	xor    %r13d,%r13d
  b6e94d:	4d 3b 2c 24          	cmp    (%r12),%r13
  b6e951:	0f 85 1e fe ff ff    	jne    b6e775 <badger_runtime::gc::alloc_uninit+0x3c5>
  b6e957:	4c 89 e7             	mov    %r12,%rdi
  b6e95a:	ff 15 18 34 ed 00    	call   *0xed3418(%rip)        # 1a41d78 <_DYNAMIC+0x3688>
  b6e960:	e9 10 fe ff ff       	jmp    b6e775 <badger_runtime::gc::alloc_uninit+0x3c5>
  b6e965:	e8 68 75 f5 ff       	call   ac5ed2 <<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::initialize::<<std::sync::once_lock::OnceLock<core::option::Option<u64>>>::get_or_init<badger_runtime::task::chaos::{closure#0}>::{closure#0}, !>>
  b6e96a:	64 48 8b 04 25 28 fb 	mov    %fs:0xfffffffffffffb28,%rax
  b6e971:	ff ff 
  b6e973:	48 85 c0             	test   %rax,%rax
  b6e976:	0f 85 b3 fa ff ff    	jne    b6e42f <badger_runtime::gc::alloc_uninit+0x7f>
  b6e97c:	e9 7b fa ff ff       	jmp    b6e3fc <badger_runtime::gc::alloc_uninit+0x4c>
  b6e981:	e8 89 7e f5 ff       	call   ac680f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b6e986:	0f b6 05 7f 30 ee 00 	movzbl 0xee307f(%rip),%eax        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b6e98d:	40 08 e8             	or     %bpl,%al
  b6e990:	3c 01                	cmp    $0x1,%al
  b6e992:	0f 84 57 fb ff ff    	je     b6e4ef <badger_runtime::gc::alloc_uninit+0x13f>
  b6e998:	e9 4d fc ff ff       	jmp    b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6e99d:	e8 6d 7e f5 ff       	call   ac680f <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::stress::{closure#0}>::{closure#0}, !>>
  b6e9a2:	40 0a 2d 63 30 ee 00 	or     0xee3063(%rip),%bpl        # 1a51a0c <badger_runtime::gc::stress::STRESS+0x4>
  b6e9a9:	40 80 fd 01          	cmp    $0x1,%bpl
  b6e9ad:	0f 85 4f ff ff ff    	jne    b6e902 <badger_runtime::gc::alloc_uninit+0x552>
  b6e9b3:	bf 01 00 00 00       	mov    $0x1,%edi
  b6e9b8:	e8 93 c4 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b6e9bd:	49 89 c7             	mov    %rax,%r15
  b6e9c0:	89 d5                	mov    %edx,%ebp
  b6e9c2:	ff 15 f8 40 ed 00    	call   *0xed40f8(%rip)        # 1a42ac0 <_DYNAMIC+0x43d0>
  b6e9c8:	e8 43 08 00 00       	call   b6f210 <badger_runtime::gc::collect_inner>
  b6e9cd:	45 31 f6             	xor    %r14d,%r14d
  b6e9d0:	31 c0                	xor    %eax,%eax
  b6e9d2:	86 05 38 bb ee 00    	xchg   %al,0xeebb38(%rip)        # 1a5a510 <badger_runtime::task::STOP>
  b6e9d8:	bf 01 00 00 00       	mov    $0x1,%edi
  b6e9dd:	e8 6e c4 a7 00       	call   15eae50 <<std::sys::pal::unix::time::Timespec>::now>
  b6e9e2:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  b6e9e7:	89 54 24 30          	mov    %edx,0x30(%rsp)
  b6e9eb:	4c 89 7c 24 38       	mov    %r15,0x38(%rsp)
  b6e9f0:	89 6c 24 40          	mov    %ebp,0x40(%rsp)
  b6e9f4:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  b6e9f9:	48 8d 74 24 28       	lea    0x28(%rsp),%rsi
  b6e9fe:	48 8d 54 24 38       	lea    0x38(%rsp),%rdx
  b6ea03:	e8 78 c3 a7 00       	call   15ead80 <<std::sys::pal::unix::time::Timespec>::sub_timespec>
  b6ea08:	8b 44 24 18          	mov    0x18(%rsp),%eax
  b6ea0c:	48 69 4c 24 10 00 ca 	imul   $0x3b9aca00,0x10(%rsp),%rcx
  b6ea13:	9a 3b 
  b6ea15:	48 01 c1             	add    %rax,%rcx
  b6ea18:	80 7c 24 08 00       	cmpb   $0x0,0x8(%rsp)
  b6ea1d:	4c 0f 44 f1          	cmove  %rcx,%r14
  b6ea21:	48 8b 05 00 0f ed 00 	mov    0xed0f00(%rip),%rax        # 1a3f928 <_DYNAMIC+0x1238>
  b6ea28:	4c 01 b0 c0 00 00 00 	add    %r14,0xc0(%rax)
  b6ea2f:	48 8b 88 c8 00 00 00 	mov    0xc8(%rax),%rcx
  b6ea36:	49 39 ce             	cmp    %rcx,%r14
  b6ea39:	4c 0f 46 f1          	cmovbe %rcx,%r14
  b6ea3d:	4c 89 b0 c8 00 00 00 	mov    %r14,0xc8(%rax)
  b6ea44:	48 8b 05 b5 2c ee 00 	mov    0xee2cb5(%rip),%rax        # 1a51700 <badger_runtime::task::WORKERS>
  b6ea4b:	48 83 f8 02          	cmp    $0x2,%rax
  b6ea4f:	0f 82 95 fb ff ff    	jb     b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6ea55:	ff 15 e5 30 ed 00    	call   *0xed30e5(%rip)        # 1a41b40 <_DYNAMIC+0x3450>
  b6ea5b:	48 83 f8 40          	cmp    $0x40,%rax
  b6ea5f:	0f 83 cb 00 00 00    	jae    b6eb30 <badger_runtime::gc::alloc_uninit+0x780>
  b6ea65:	48 c1 e0 07          	shl    $0x7,%rax
  b6ea69:	48 8d 0d 90 9a ee 00 	lea    0xee9a90(%rip),%rcx        # 1a58500 <badger_runtime::task::IN_COMPILED>
  b6ea70:	f0 48 ff 04 08       	lock incq (%rax,%rcx,1)
  b6ea75:	e8 46 39 02 00       	call   b923c0 <badger_runtime::task::unlock>
  b6ea7a:	e8 c1 a5 01 00       	call   b89040 <badger_runtime::task::chaos_jitter>
  b6ea7f:	e9 66 fb ff ff       	jmp    b6e5ea <badger_runtime::gc::alloc_uninit+0x23a>
  b6ea84:	48 8d 15 bd 3b e5 00 	lea    0xe53bbd(%rip),%rdx        # 19c2648 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0xf20>
  b6ea8b:	be 40 00 00 00       	mov    $0x40,%esi
  b6ea90:	4c 89 ff             	mov    %r15,%rdi
  b6ea93:	ff 15 b7 fe ec 00    	call   *0xecfeb7(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6ea99:	48 8d 3d 4b f0 6f ff 	lea    -0x900fb5(%rip),%rdi        # 26daeb <anon.5680abfc9ce5520a59545b4df02fd4ad.533.llvm.1751364238661663638>
  b6eaa0:	48 8d 0d d1 46 e5 00 	lea    0xe546d1(%rip),%rcx        # 19c3178 <anon.5680abfc9ce5520a59545b4df02fd4ad.563.llvm.1751364238661663638+0x160>
  b6eaa7:	4c 8d 05 82 56 e5 00 	lea    0xe55682(%rip),%r8        # 19c4130 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x1a0>
  b6eaae:	48 8d 54 24 07       	lea    0x7(%rsp),%rdx
  b6eab3:	be 2b 00 00 00       	mov    $0x2b,%esi
  b6eab8:	ff 15 d2 ff ec 00    	call   *0xecffd2(%rip)        # 1a3ea90 <_DYNAMIC+0x3a0>
  b6eabe:	48 8d 15 13 2e e5 00 	lea    0xe52e13(%rip),%rdx        # 19c18d8 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0x1b0>
  b6eac5:	be 40 00 00 00       	mov    $0x40,%esi
  b6eaca:	4c 89 ff             	mov    %r15,%rdi
  b6eacd:	ff 15 7d fe ec 00    	call   *0xecfe7d(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6ead3:	48 8d 15 16 2e e5 00 	lea    0xe52e16(%rip),%rdx        # 19c18f0 <anon.5680abfc9ce5520a59545b4df02fd4ad.67.llvm.1751364238661663638+0x1c8>
  b6eada:	be 20 00 00 00       	mov    $0x20,%esi
  b6eadf:	48 c7 c7 ff ff ff ff 	mov    $0xffffffffffffffff,%rdi
  b6eae6:	ff 15 64 fe ec 00    	call   *0xecfe64(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6eaec:	48 8d 3d f9 e9 6f ff 	lea    -0x901607(%rip),%rdi        # 26d4ec <anon.3a879afe13029b9ad6f5d17cd43d38ab.43.llvm.1747630503651334518+0x135aa>
  b6eaf3:	be 2b 00 00 00       	mov    $0x2b,%esi
  b6eaf8:	ff 15 02 06 ed 00    	call   *0xed0602(%rip)        # 1a3f100 <_DYNAMIC+0xa10>
  b6eafe:	ff 15 9c 06 ed 00    	call   *0xed069c(%rip)        # 1a3f1a0 <_DYNAMIC+0xab0>
  b6eb04:	48 8d 15 e5 54 e5 00 	lea    0xe554e5(%rip),%rdx        # 19c3ff0 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x60>
  b6eb0b:	be 40 00 00 00       	mov    $0x40,%esi
  b6eb10:	48 89 c7             	mov    %rax,%rdi
  b6eb13:	ff 15 37 fe ec 00    	call   *0xecfe37(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6eb19:	48 8d 15 60 59 e5 00 	lea    0xe55960(%rip),%rdx        # 19c4480 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x4f0>
  b6eb20:	be 40 00 00 00       	mov    $0x40,%esi
  b6eb25:	48 89 c7             	mov    %rax,%rdi
  b6eb28:	ff 15 22 fe ec 00    	call   *0xecfe22(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6eb2e:	eb 15                	jmp    b6eb45 <badger_runtime::gc::alloc_uninit+0x795>
  b6eb30:	48 8d 15 49 59 e5 00 	lea    0xe55949(%rip),%rdx        # 19c4480 <anon.5680abfc9ce5520a59545b4df02fd4ad.1087.llvm.1751364238661663638+0x4f0>
  b6eb37:	be 40 00 00 00       	mov    $0x40,%esi
  b6eb3c:	48 89 c7             	mov    %rax,%rdi
  b6eb3f:	ff 15 0b fe ec 00    	call   *0xecfe0b(%rip)        # 1a3e950 <_DYNAMIC+0x260>
  b6eb45:	0f 0b                	ud2
  b6eb47:	ff 15 0b 05 ed 00    	call   *0xed050b(%rip)        # 1a3f058 <_DYNAMIC+0x968>
  b6eb4d:	ff 15 05 05 ed 00    	call   *0xed0505(%rip)        # 1a3f058 <_DYNAMIC+0x968>
