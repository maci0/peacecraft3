int bcj_x86_de_compress (int encoding, CALLBACK_FUNC *callback, void *auxdata)
{
  UInt32 state;  x86_Convert_Init(state);         // состояние энкодера/декодера для BCJ-X86 препроцессора
  UInt32 ip = 0;                                  // эмулируемый "счётчик команд"
  BYTE* Buf = (BYTE*) malloc(LARGE_BUFFER_SIZE);  // место для данных
  if (Buf==NULL)   return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
  int RemainderSize=0;                           // остаток данных с предыдущего раза
  int x, InSize;                                 // количество прочитанных байт
  while ( (InSize = x = callback ("read", Buf+RemainderSize, LARGE_BUFFER_SIZE-RemainderSize, auxdata)) >= 0 )
  {
    if ((InSize+=RemainderSize)==0)    goto Ok;  // Данных больше нет
    int OutSize = InSize<=5? InSize : x86_Convert(Buf, InSize, ip, &state, encoding);  // Меньше 5 байт этот фильтр не воспринимает :)
    ip += OutSize;
    if( (x=callback("write",Buf,OutSize,auxdata)) != OutSize )      goto Error;
    RemainderSize = InSize-OutSize;
    // Перенесём необработанный остаток данных в начало буфера
    if (RemainderSize>0)                memmove(Buf,Buf+OutSize,RemainderSize);
  }
Error: free(Buf); return x;            // произошла ошибка при чтении/записи
Ok:    free(Buf); return FREEARC_OK;   // всё в порядке
}
