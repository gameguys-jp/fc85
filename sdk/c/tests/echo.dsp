; echo.dsp：C の周辺の関数の試験（sdk/c/test_devices.py）の DSP のプログラム。dspasm --c-array で C の見出しにし、
; tests/dev_test.c が dsp_load() で書く。メールボックスの語に 1 を足して応答に返すだけ。

        INCLUDE "dsp.inc"

        RSSET   0
STAT    RS      1
CMD     RS      1
KCMD    RS      1
ONE     RS      1

        ORG     DSP_VEC_RESET
        B       init
        ORG     DSP_VEC_INT
        RET

init    DINT
        LDPK    0
        LACK    DSP_IS_COMMAND
        SACL    KCMD
        LACK    1
        SACL    ONE
idle    IN      STAT,DSP_PA_STATUS
        LAC     STAT
        AND     KCMD
        BZ      idle
        IN      CMD,DSP_PA_MAILBOX      ; 読むと COMMAND が下がる
        LAC     CMD
        ADD     ONE
        SACL    CMD
        OUT     CMD,DSP_PA_RESPONSE
        B       idle
