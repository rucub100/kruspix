// TODO delete this file if not used

pub enum PropValue {
    Empty,
    U32,
    U64,
    String,
    PropEncodedArray,
    PHandle,
    StringList,
}

pub enum StandardProp {
    Compatible,
    Model,
    PHandle,
    Status,
    AddressCells,
    SizeCells,
    Reg,
    VirtualReg,
    Ranges,
    DmaRanges,
    DmaCoherent,
    DmaNoncoherent,
}

pub enum InterruptGenDevProp {
    Interrupts,
    InterruptParent,
    InterruptsExtended,
}

pub enum InterruptControllersProp {
    InterruptCells,
    InterruptController
}

pub enum InterruptNexusProp {
    InterruptMap,
    InterruptMapMask,
    InterruptCells
}