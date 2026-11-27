! Synthetic PML export of one line with p-points (finding 02: ports, connectedTo, TUBI).
! Pump P-101 -> flange, elbow, support, tee, valve -> tank T-102; branch B2 leaves the tee.
SITE /SITE-A
  ZONE /SITE-A/ZONE-A
    EQUI /SITE-A/ZONE-A/P-101
      TAG 'P-101'
      DTYP PUMP
      NOZZ /SITE-A/ZONE-A/P-101/N1
        TAG 'N1'
        HBOR 100
        POS 1000.0 0.0 500.0
        HDIR -1.0 0.0 0.0
    EQUI /SITE-A/ZONE-A/T-102
      TAG 'T-102'
      DTYP TANK
      NOZZ /SITE-A/ZONE-A/T-102/N1
        TAG 'N1'
        HBOR 100
        POS 150.0 0.0 3000.0
        HDIR 0.0 0.0 -1.0
    PIPE /SITE-A/ZONE-A/PIPE-100-A
      LINREF '4"-PW-100-A1A2'
      BORE 100
      SPEC '/SPEC-A1A2'
      BRAN /SITE-A/ZONE-A/PIPE-100-A/B1
        HPOS 1000.0 0.0 500.0
        HBOR 100
        HREF /SITE-A/ZONE-A/P-101/N1
        TPOS 150.0 0.0 3000.0
        TREF /SITE-A/ZONE-A/T-102/N1
        FLAN /SITE-A/ZONE-A/PIPE-100-A/B1/FLAN-1
          SKEY FLWNRF
          BORE 100
          APOS 1000.0 0.0 500.0
          ADIR 1.0 0.0 0.0
          ACON FBD
          LPOS 900.0 0.0 500.0
          LDIR -1.0 0.0 0.0
          LCON BWD
        ELBO /SITE-A/ZONE-A/PIPE-100-A/B1/ELBO-1
          SKEY ELBWLR90
          BORE 100
          ANGL 90.0
          RADI 150.0
          APOS 300.0 0.0 500.0
          ADIR 1.0 0.0 0.0
          ACON BWD
          LPOS 150.0 0.0 650.0
          LDIR 0.0 0.0 1.0
          LCON BWD
        PSUP /SITE-A/ZONE-A/PIPE-100-A/B1/PS-1
          SKEY SUPRW
          BORE 100
          POS 150.0 0.0 1000.0
        TEE /SITE-A/ZONE-A/PIPE-100-A/B1/TEE-1
          SKEY TEBW
          BORE 100
          APOS 150.0 0.0 1500.0
          ADIR 0.0 0.0 -1.0
          ACON BWD
          LPOS 150.0 0.0 1800.0
          LDIR 0.0 0.0 1.0
          LCON BWD
          P3POS 150.0 150.0 1650.0
          P3DIR 0.0 1.0 0.0
          P3BOR 50
          P3CON BWD
        VALV /SITE-A/ZONE-A/PIPE-100-A/B1/VALV-1
          SKEY GTFLFL
          TAG 'HV-101'
          BORE 100
          APOS 150.0 0.0 2000.0
          ADIR 0.0 0.0 -1.0
          ACON FBD
          LPOS 150.0 0.0 2300.0
          LDIR 0.0 0.0 1.0
          LCON FBD
      BRAN /SITE-A/ZONE-A/PIPE-100-A/B2
        HPOS 150.0 150.0 1650.0
        HBOR 50
        HREF /SITE-A/ZONE-A/PIPE-100-A/B1/TEE-1
        TPOS 150.0 450.0 1650.0
        VALV /SITE-A/ZONE-A/PIPE-100-A/B2/VALV-2
          SKEY BLSWSW
          BORE 50
          APOS 150.0 300.0 1650.0
          ADIR 0.0 -1.0 0.0
          ACON SWF
          LPOS 150.0 450.0 1650.0
          LDIR 0.0 1.0 0.0
          LCON SWF
