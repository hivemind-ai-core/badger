
/home/dan/orca/projects/language/target/memory-binaries-2026-10-18/buffer-fixed-candidate:     file format elf64-x86-64


Disassembly of section .text:

0000000000bb1f70 <badger_runtime::hamt::get>:
  bb1f70:	55                   	push   %rbp
  bb1f71:	41 57                	push   %r15
  bb1f73:	41 56                	push   %r14
  bb1f75:	41 55                	push   %r13
  bb1f77:	41 54                	push   %r12
  bb1f79:	53                   	push   %rbx
  bb1f7a:	48 83 ec 18          	sub    $0x18,%rsp
  bb1f7e:	48 89 4c 24 08       	mov    %rcx,0x8(%rsp)
  bb1f83:	48 89 54 24 10       	mov    %rdx,0x10(%rsp)
  bb1f88:	48 85 ff             	test   %rdi,%rdi
  bb1f8b:	0f 84 e2 01 00 00    	je     bb2173 <badger_runtime::hamt::get+0x203>
  bb1f91:	49 89 f4             	mov    %rsi,%r12
  bb1f94:	48 89 fb             	mov    %rdi,%rbx
  bb1f97:	8b 05 f3 33 ee 00    	mov    0xee33f3(%rip),%eax        # 1a95390 <badger_runtime::gc::verify::VERIFY>
  bb1f9d:	85 c0                	test   %eax,%eax
  bb1f9f:	0f 85 eb 01 00 00    	jne    bb2190 <badger_runtime::hamt::get+0x220>
  bb1fa5:	80 3d e8 33 ee 00 01 	cmpb   $0x1,0xee33e8(%rip)        # 1a95394 <badger_runtime::gc::verify::VERIFY+0x4>
  bb1fac:	75 1e                	jne    bb1fcc <badger_runtime::hamt::get+0x5c>
  bb1fae:	4c 8b 33             	mov    (%rbx),%r14
  bb1fb1:	8b 05 c9 33 ee 00    	mov    0xee33c9(%rip),%eax        # 1a95380 <badger_runtime::gc::poison::D+0x8>
  bb1fb7:	85 c0                	test   %eax,%eax
  bb1fb9:	0f 85 ee 01 00 00    	jne    bb21ad <badger_runtime::hamt::get+0x23d>
  bb1fbf:	4c 3b 35 b2 33 ee 00 	cmp    0xee33b2(%rip),%r14        # 1a95378 <badger_runtime::gc::poison::D>
  bb1fc6:	0f 84 f9 01 00 00    	je     bb21c5 <badger_runtime::hamt::get+0x255>
  bb1fcc:	48 83 c3 10          	add    $0x10,%rbx
  bb1fd0:	48 c7 c5 fb ff ff ff 	mov    $0xfffffffffffffffb,%rbp
  bb1fd7:	45 31 ed             	xor    %r13d,%r13d
  bb1fda:	eb 23                	jmp    bb1fff <badger_runtime::hamt::get+0x8f>
  bb1fdc:	0f 1f 40 00          	nopl   0x0(%rax)
  bb1fe0:	4c 3b 35 91 33 ee 00 	cmp    0xee3391(%rip),%r14        # 1a95378 <badger_runtime::gc::poison::D>
  bb1fe7:	0f 84 d8 01 00 00    	je     bb21c5 <badger_runtime::hamt::get+0x255>
  bb1fed:	48 83 c3 10          	add    $0x10,%rbx
  bb1ff1:	48 83 c5 05          	add    $0x5,%rbp
  bb1ff5:	48 83 fd 3a          	cmp    $0x3a,%rbp
  bb1ff9:	0f 87 22 01 00 00    	ja     bb2121 <badger_runtime::hamt::get+0x1b1>
  bb1fff:	8d 4d 05             	lea    0x5(%rbp),%ecx
  bb2002:	4c 89 e0             	mov    %r12,%rax
  bb2005:	48 d3 e8             	shr    %cl,%rax
  bb2008:	ba 01 00 00 00       	mov    $0x1,%edx
  bb200d:	89 c1                	mov    %eax,%ecx
  bb200f:	d3 e2                	shl    %cl,%edx
  bb2011:	8b 0b                	mov    (%rbx),%ecx
  bb2013:	0f a3 c1             	bt     %eax,%ecx
  bb2016:	0f 82 a3 00 00 00    	jb     bb20bf <badger_runtime::hamt::get+0x14f>
  bb201c:	8b 43 04             	mov    0x4(%rbx),%eax
  bb201f:	85 c2                	test   %eax,%edx
  bb2021:	0f 84 4f 01 00 00    	je     bb2176 <badger_runtime::hamt::get+0x206>
  bb2027:	ff ca                	dec    %edx
  bb2029:	21 c2                	and    %eax,%edx
  bb202b:	89 d0                	mov    %edx,%eax
  bb202d:	d1 e8                	shr    $1,%eax
  bb202f:	25 55 55 55 55       	and    $0x55555555,%eax
  bb2034:	29 c2                	sub    %eax,%edx
  bb2036:	89 d0                	mov    %edx,%eax
  bb2038:	25 33 33 33 33       	and    $0x33333333,%eax
  bb203d:	c1 ea 02             	shr    $0x2,%edx
  bb2040:	81 e2 33 33 33 33    	and    $0x33333333,%edx
  bb2046:	01 c2                	add    %eax,%edx
  bb2048:	89 d0                	mov    %edx,%eax
  bb204a:	c1 e8 04             	shr    $0x4,%eax
  bb204d:	01 d0                	add    %edx,%eax
  bb204f:	25 0f 0f 0f 0f       	and    $0xf0f0f0f,%eax
  bb2054:	69 c0 01 01 01 01    	imul   $0x1010101,%eax,%eax
  bb205a:	8b 4b 0c             	mov    0xc(%rbx),%ecx
  bb205d:	48 8d 0c 49          	lea    (%rcx,%rcx,2),%rcx
  bb2061:	c1 e8 18             	shr    $0x18,%eax
  bb2064:	48 8d 0c cb          	lea    (%rbx,%rcx,8),%rcx
  bb2068:	48 8b 5c c1 10       	mov    0x10(%rcx,%rax,8),%rbx
  bb206d:	8b 05 1d 33 ee 00    	mov    0xee331d(%rip),%eax        # 1a95390 <badger_runtime::gc::verify::VERIFY>
  bb2073:	85 c0                	test   %eax,%eax
  bb2075:	75 2e                	jne    bb20a5 <badger_runtime::hamt::get+0x135>
  bb2077:	80 3d 16 33 ee 00 01 	cmpb   $0x1,0xee3316(%rip)        # 1a95394 <badger_runtime::gc::verify::VERIFY+0x4>
  bb207e:	0f 85 69 ff ff ff    	jne    bb1fed <badger_runtime::hamt::get+0x7d>
  bb2084:	4c 8b 33             	mov    (%rbx),%r14
  bb2087:	8b 05 f3 32 ee 00    	mov    0xee32f3(%rip),%eax        # 1a95380 <badger_runtime::gc::poison::D+0x8>
  bb208d:	85 c0                	test   %eax,%eax
  bb208f:	0f 84 4b ff ff ff    	je     bb1fe0 <badger_runtime::hamt::get+0x70>
  bb2095:	4d 89 c7             	mov    %r8,%r15
  bb2098:	e8 48 88 f4 ff       	call   afa8e5 <<std::sync::once_lock::OnceLock<&badger_runtime::desc::TypeDesc>>::initialize::<<std::sync::once_lock::OnceLock<&badger_runtime::desc::TypeDesc>>::get_or_init<badger_runtime::gc::poison::{closure#0}>::{closure#0}, !>>
  bb209d:	4d 89 f8             	mov    %r15,%r8
  bb20a0:	e9 3b ff ff ff       	jmp    bb1fe0 <badger_runtime::hamt::get+0x70>
  bb20a5:	4d 89 c6             	mov    %r8,%r14
  bb20a8:	e8 b4 8c f4 ff       	call   afad61 <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::verify::{closure#0}>::{closure#0}, !>>
  bb20ad:	4d 89 f0             	mov    %r14,%r8
  bb20b0:	80 3d dd 32 ee 00 01 	cmpb   $0x1,0xee32dd(%rip)        # 1a95394 <badger_runtime::gc::verify::VERIFY+0x4>
  bb20b7:	0f 85 30 ff ff ff    	jne    bb1fed <badger_runtime::hamt::get+0x7d>
  bb20bd:	eb c5                	jmp    bb2084 <badger_runtime::hamt::get+0x114>
  bb20bf:	ff ca                	dec    %edx
  bb20c1:	21 ca                	and    %ecx,%edx
  bb20c3:	89 d0                	mov    %edx,%eax
  bb20c5:	d1 e8                	shr    $1,%eax
  bb20c7:	25 55 55 55 55       	and    $0x55555555,%eax
  bb20cc:	29 c2                	sub    %eax,%edx
  bb20ce:	89 d0                	mov    %edx,%eax
  bb20d0:	25 33 33 33 33       	and    $0x33333333,%eax
  bb20d5:	c1 ea 02             	shr    $0x2,%edx
  bb20d8:	81 e2 33 33 33 33    	and    $0x33333333,%edx
  bb20de:	01 c2                	add    %eax,%edx
  bb20e0:	89 d0                	mov    %edx,%eax
  bb20e2:	c1 e8 04             	shr    $0x4,%eax
  bb20e5:	01 d0                	add    %edx,%eax
  bb20e7:	25 0f 0f 0f 0f       	and    $0xf0f0f0f,%eax
  bb20ec:	69 c0 01 01 01 01    	imul   $0x1010101,%eax,%eax
  bb20f2:	c1 e8 15             	shr    $0x15,%eax
  bb20f5:	83 e0 f8             	and    $0xfffffff8,%eax
  bb20f8:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  bb20fc:	4c 39 64 03 10       	cmp    %r12,0x10(%rbx,%rax,1)
  bb2101:	75 70                	jne    bb2173 <badger_runtime::hamt::get+0x203>
  bb2103:	48 8b 74 03 18       	mov    0x18(%rbx,%rax,1),%rsi
  bb2108:	48 8b 6c 03 20       	mov    0x20(%rbx,%rax,1),%rbp
  bb210d:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  bb2112:	48 8b 54 24 10       	mov    0x10(%rsp),%rdx
  bb2117:	41 ff 50 28          	call   *0x28(%r8)
  bb211b:	44 0f b6 e8          	movzbl %al,%r13d
  bb211f:	eb 55                	jmp    bb2176 <badger_runtime::hamt::get+0x206>
  bb2121:	8b 43 0c             	mov    0xc(%rbx),%eax
  bb2124:	48 85 c0             	test   %rax,%rax
  bb2127:	74 4a                	je     bb2173 <badger_runtime::hamt::get+0x203>
  bb2129:	4d 8b 70 28          	mov    0x28(%r8),%r14
  bb212d:	48 c1 e0 03          	shl    $0x3,%rax
  bb2131:	4c 8d 3c 40          	lea    (%rax,%rax,2),%r15
  bb2135:	45 31 ed             	xor    %r13d,%r13d
  bb2138:	eb 0f                	jmp    bb2149 <badger_runtime::hamt::get+0x1d9>
  bb213a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
  bb2140:	49 83 c5 18          	add    $0x18,%r13
  bb2144:	4d 39 ef             	cmp    %r13,%r15
  bb2147:	74 42                	je     bb218b <badger_runtime::hamt::get+0x21b>
  bb2149:	4e 39 64 2b 10       	cmp    %r12,0x10(%rbx,%r13,1)
  bb214e:	75 f0                	jne    bb2140 <badger_runtime::hamt::get+0x1d0>
  bb2150:	4a 8b 74 2b 18       	mov    0x18(%rbx,%r13,1),%rsi
  bb2155:	4a 8b 6c 2b 20       	mov    0x20(%rbx,%r13,1),%rbp
  bb215a:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  bb215f:	48 8b 54 24 10       	mov    0x10(%rsp),%rdx
  bb2164:	41 ff d6             	call   *%r14
  bb2167:	84 c0                	test   %al,%al
  bb2169:	74 d5                	je     bb2140 <badger_runtime::hamt::get+0x1d0>
  bb216b:	41 bd 01 00 00 00    	mov    $0x1,%r13d
  bb2171:	eb 03                	jmp    bb2176 <badger_runtime::hamt::get+0x206>
  bb2173:	45 31 ed             	xor    %r13d,%r13d
  bb2176:	4c 89 e8             	mov    %r13,%rax
  bb2179:	48 89 ea             	mov    %rbp,%rdx
  bb217c:	48 83 c4 18          	add    $0x18,%rsp
  bb2180:	5b                   	pop    %rbx
  bb2181:	41 5c                	pop    %r12
  bb2183:	41 5d                	pop    %r13
  bb2185:	41 5e                	pop    %r14
  bb2187:	41 5f                	pop    %r15
  bb2189:	5d                   	pop    %rbp
  bb218a:	c3                   	ret
  bb218b:	45 31 ed             	xor    %r13d,%r13d
  bb218e:	eb e6                	jmp    bb2176 <badger_runtime::hamt::get+0x206>
  bb2190:	4d 89 c6             	mov    %r8,%r14
  bb2193:	e8 c9 8b f4 ff       	call   afad61 <<std::sync::once_lock::OnceLock<bool>>::initialize::<<std::sync::once_lock::OnceLock<bool>>::get_or_init<badger_runtime::gc::verify::{closure#0}>::{closure#0}, !>>
  bb2198:	4d 89 f0             	mov    %r14,%r8
  bb219b:	80 3d f2 31 ee 00 01 	cmpb   $0x1,0xee31f2(%rip)        # 1a95394 <badger_runtime::gc::verify::VERIFY+0x4>
  bb21a2:	0f 84 06 fe ff ff    	je     bb1fae <badger_runtime::hamt::get+0x3e>
  bb21a8:	e9 1f fe ff ff       	jmp    bb1fcc <badger_runtime::hamt::get+0x5c>
  bb21ad:	4d 89 c7             	mov    %r8,%r15
  bb21b0:	e8 30 87 f4 ff       	call   afa8e5 <<std::sync::once_lock::OnceLock<&badger_runtime::desc::TypeDesc>>::initialize::<<std::sync::once_lock::OnceLock<&badger_runtime::desc::TypeDesc>>::get_or_init<badger_runtime::gc::poison::{closure#0}>::{closure#0}, !>>
  bb21b5:	4d 89 f8             	mov    %r15,%r8
  bb21b8:	4c 3b 35 b9 31 ee 00 	cmp    0xee31b9(%rip),%r14        # 1a95378 <badger_runtime::gc::poison::D>
  bb21bf:	0f 85 07 fe ff ff    	jne    bb1fcc <badger_runtime::hamt::get+0x5c>
  bb21c5:	48 8d 35 41 06 6c ff 	lea    -0x93f9bf(%rip),%rsi        # 27280d <anon.b8ca5138f52a8f49a92309e565734610.1182.llvm.14156111013822978802>
  bb21cc:	ba 0b 00 00 00       	mov    $0xb,%edx
  bb21d1:	48 89 df             	mov    %rbx,%rdi
  bb21d4:	e8 57 a7 ff ff       	call   bac930 <badger_runtime::gc::freed_use>
