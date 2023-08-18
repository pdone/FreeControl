@echo off
set scrcpy_path=%APPDATA%\FreeControl\%1
color 0E
if not exist "%scrcpy_path%\adb.exe" (echo No found adb.exe
echo Press any key to exit&&pause>nul&&exit
)
mode con lines=45 cols=90
cd %scrcpy_path%
echo Loading device list
adb devices
echo Setting port
adb tcpip 5555
set res="%ERRORLEVEL%"
if %res%=="0" (
echo "          .,:,,,                                        .::,,,::."
echo "        .::::,,;;,                                  .,;;:,,....:i:"
echo "        :i,.::::,;i:.      ....,,:::::::::,....   .;i:,.  ......;i."
echo "        :;..:::;::::i;,,:::;:,,,,,,,,,,..,.,,:::iri:. .,:irsr:,.;i."
echo "        ;;..,::::;;;;ri,,,.                    ..,,:;s1s1ssrr;,.;r,"
echo "        :;. ,::;ii;:,     . ...................     .;iirri;;;,,;i,"
echo "        ,i. .;ri:.   ... ............................  .,,:;:,,,;i:"
echo "        :s,.;r:... ....................................... .::;::s;"
echo "        ,1r::. .............,,,.,,:,,........................,;iir;"
echo "        ,s;...........     ..::.,;:,,.          ...............,;1s"
echo "       :i,..,.              .,:,,::,.          .......... .......;1,"
echo "      ir,....:rrssr;:,       ,,.,::.     .r5S9989398G95hr;. ....,.:s,"
echo "     ;r,..,s9855513XHAG3i   .,,,,,,,.  ,S931,.,,.;s;s&BHHA8s.,..,..:r:"
echo "    :r;..rGGh,  :SAG;;G@BS:.,,,,,,,,,.r83:      hHH1sXMBHHHM3..,,,,.ir."
echo "   ,si,.1GS,   sBMAAX&MBMB5,,,,,,:,,.:&8       3@HXHBMBHBBH#X,.,,,,,,rr"
echo "   ;1:,,SH:   .A@&&B#&8H#BS,,,,,,,,,.,5XS,     3@MHABM&59M#As..,,,,:,is,"
echo "  .rr,,,;9&1   hBHHBB&8AMGr,,,,,,,,,,,:h&&9s;   r9&BMHBHMB9:  . .,,,,;ri."
echo "  :1:....:5&XSi;r8BMBHHA9r:,......,,,,:ii19GG88899XHHH&GSr.      ...,:rs."
echo "  ;s.     .:sS8G8GG889hi.        ....,,:;:,.:irssrriii:,.        ...,,i1,"
echo "  ;1,         ..,....,,isssi;,        .,,.                      ....,.i1,"
echo "  ;h:               i9HHBMBBHAX9:         .                     ...,,,rs,"
echo "  ,1i..            :A#MBBBBMHB##s                             ....,,,;si."
echo "  .r1,..        ,..;3BMBBBHBB#Bh.     ..                    ....,,,,,i1;"
echo "   :h;..       .,..;,1XBMMMMBXs,.,, .. :: ,.               ....,,,,,,ss."
echo "    ih: ..    .;;;, ;;:s58A3i,..    ,. ,.:,,.             ...,,,,,:,s1,"
echo "    .s1,....   .,;sh,  ,iSAXs;.    ,.  ,,.i85            ...,,,,,,:i1;"
echo "     .rh: ...     rXG9XBBM#M#MHAX3hss13&&HHXr         .....,,,,,,,ih;"
echo "      .s5: .....    i598X&&A&AAAAAA&XG851r:       ........,,,,:,,sh;"
echo "      . ihr, ...  .         ..                    ........,,,,,;11:."
echo "         ,s1i. ...  ..,,,..,,,.,,.,,.,..       ........,,.,,.;s5i."
echo "          .:s1r,......................       ..............;shs,"
echo "          . .:shr:.  ....                 ..............,ishs."
echo "              .,issr;,... ...........................,is1s;."
echo "                 .,is1si;:,....................,:;ir1sr;,"
echo "                    ..:isssssrrii;::::::;;iirsssssr;:.."
echo "                         .,::iiirsssssssssrri;;:."
color 2
echo Success! Press any key to exit&&pause>nul&&exit
)
color 4
echo Faild! Press any key to exit&&pause>nul&&exit
