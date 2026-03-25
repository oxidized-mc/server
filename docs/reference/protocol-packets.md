# Protocol Packets Reference (26.1)

Extracted from the decompiled vanilla server JAR.

---

## Handshaking

| Direction | Packet Class |
|---|---|
| Serverbound | `ClientIntentionPacket` |

## Status

| Direction | Packet Class |
|---|---|
| Clientbound | `ClientboundStatusResponsePacket` |
| Serverbound | `ServerboundStatusRequestPacket` |

## Login

| Direction | Packet Class |
|---|---|
| Clientbound | `ClientboundCustomQueryPacket` |
| Clientbound | `ClientboundHelloPacket` |
| Clientbound | `ClientboundLoginCompressionPacket` |
| Clientbound | `ClientboundLoginDisconnectPacket` |
| Clientbound | `ClientboundLoginFinishedPacket` |
| Serverbound | `ServerboundCustomQueryAnswerPacket` |
| Serverbound | `ServerboundHelloPacket` |
| Serverbound | `ServerboundKeyPacket` |
| Serverbound | `ServerboundLoginAcknowledgedPacket` |

## Configuration

| Direction | Packet Class |
|---|---|
| Clientbound | `ClientboundCodeOfConductPacket` |
| Clientbound | `ClientboundFinishConfigurationPacket` |
| Clientbound | `ClientboundRegistryDataPacket` |
| Clientbound | `ClientboundResetChatPacket` |
| Clientbound | `ClientboundSelectKnownPacks` |
| Clientbound | `ClientboundUpdateEnabledFeaturesPacket` |
| Serverbound | `ServerboundAcceptCodeOfConductPacket` |
| Serverbound | `ServerboundFinishConfigurationPacket` |
| Serverbound | `ServerboundSelectKnownPacks` |

## Play

### Clientbound (127 packets)

| Packet Class |
|---|
| `ClientboundAddEntityPacket` |
| `ClientboundAnimatePacket` |
| `ClientboundAwardStatsPacket` |
| `ClientboundBlockChangedAckPacket` |
| `ClientboundBlockDestructionPacket` |
| `ClientboundBlockEntityDataPacket` |
| `ClientboundBlockEventPacket` |
| `ClientboundBlockUpdatePacket` |
| `ClientboundBossEventPacket` |
| `ClientboundBundleDelimiterPacket` |
| `ClientboundBundlePacket` |
| `ClientboundChangeDifficultyPacket` |
| `ClientboundChunkBatchFinishedPacket` |
| `ClientboundChunkBatchStartPacket` |
| `ClientboundChunksBiomesPacket` |
| `ClientboundClearTitlesPacket` |
| `ClientboundCommandsPacket` |
| `ClientboundCommandSuggestionsPacket` |
| `ClientboundContainerClosePacket` |
| `ClientboundContainerSetContentPacket` |
| `ClientboundContainerSetDataPacket` |
| `ClientboundContainerSetSlotPacket` |
| `ClientboundCooldownPacket` |
| `ClientboundCustomChatCompletionsPacket` |
| `ClientboundDamageEventPacket` |
| `ClientboundDebugBlockValuePacket` |
| `ClientboundDebugChunkValuePacket` |
| `ClientboundDebugEntityValuePacket` |
| `ClientboundDebugEventPacket` |
| `ClientboundDebugSamplePacket` |
| `ClientboundDeleteChatPacket` |
| `ClientboundDisguisedChatPacket` |
| `ClientboundEntityEventPacket` |
| `ClientboundEntityPositionSyncPacket` |
| `ClientboundExplodePacket` |
| `ClientboundForgetLevelChunkPacket` |
| `ClientboundGameEventPacket` |
| `ClientboundGameRuleValuesPacket` |
| `ClientboundGameTestHighlightPosPacket` |
| `ClientboundHurtAnimationPacket` |
| `ClientboundInitializeBorderPacket` |
| `ClientboundLevelChunkPacketData` |
| `ClientboundLevelChunkWithLightPacket` |
| `ClientboundLevelEventPacket` |
| `ClientboundLevelParticlesPacket` |
| `ClientboundLightUpdatePacket` |
| `ClientboundLightUpdatePacketData` |
| `ClientboundLoginPacket` |
| `ClientboundLowDiskSpaceWarningPacket` |
| `ClientboundMapItemDataPacket` |
| `ClientboundMerchantOffersPacket` |
| `ClientboundMountScreenOpenPacket` |
| `ClientboundMoveEntityPacket` |
| `ClientboundMoveMinecartPacket` |
| `ClientboundMoveVehiclePacket` |
| `ClientboundOpenBookPacket` |
| `ClientboundOpenScreenPacket` |
| `ClientboundOpenSignEditorPacket` |
| `ClientboundPlaceGhostRecipePacket` |
| `ClientboundPlayerAbilitiesPacket` |
| `ClientboundPlayerChatPacket` |
| `ClientboundPlayerCombatEndPacket` |
| `ClientboundPlayerCombatEnterPacket` |
| `ClientboundPlayerCombatKillPacket` |
| `ClientboundPlayerInfoRemovePacket` |
| `ClientboundPlayerInfoUpdatePacket` |
| `ClientboundPlayerLookAtPacket` |
| `ClientboundPlayerPositionPacket` |
| `ClientboundPlayerRotationPacket` |
| `ClientboundProjectilePowerPacket` |
| `ClientboundRecipeBookAddPacket` |
| `ClientboundRecipeBookRemovePacket` |
| `ClientboundRecipeBookSettingsPacket` |
| `ClientboundRemoveEntitiesPacket` |
| `ClientboundRemoveMobEffectPacket` |
| `ClientboundResetScorePacket` |
| `ClientboundRespawnPacket` |
| `ClientboundRotateHeadPacket` |
| `ClientboundSectionBlocksUpdatePacket` |
| `ClientboundSelectAdvancementsTabPacket` |
| `ClientboundServerDataPacket` |
| `ClientboundSetActionBarTextPacket` |
| `ClientboundSetBorderCenterPacket` |
| `ClientboundSetBorderLerpSizePacket` |
| `ClientboundSetBorderSizePacket` |
| `ClientboundSetBorderWarningDelayPacket` |
| `ClientboundSetBorderWarningDistancePacket` |
| `ClientboundSetCameraPacket` |
| `ClientboundSetChunkCacheCenterPacket` |
| `ClientboundSetChunkCacheRadiusPacket` |
| `ClientboundSetCursorItemPacket` |
| `ClientboundSetDefaultSpawnPositionPacket` |
| `ClientboundSetDisplayObjectivePacket` |
| `ClientboundSetEntityDataPacket` |
| `ClientboundSetEntityLinkPacket` |
| `ClientboundSetEntityMotionPacket` |
| `ClientboundSetEquipmentPacket` |
| `ClientboundSetExperiencePacket` |
| `ClientboundSetHealthPacket` |
| `ClientboundSetHeldSlotPacket` |
| `ClientboundSetObjectivePacket` |
| `ClientboundSetPassengersPacket` |
| `ClientboundSetPlayerInventoryPacket` |
| `ClientboundSetPlayerTeamPacket` |
| `ClientboundSetScorePacket` |
| `ClientboundSetSimulationDistancePacket` |
| `ClientboundSetSubtitleTextPacket` |
| `ClientboundSetTimePacket` |
| `ClientboundSetTitlesAnimationPacket` |
| `ClientboundSetTitleTextPacket` |
| `ClientboundSoundEntityPacket` |
| `ClientboundSoundPacket` |
| `ClientboundStartConfigurationPacket` |
| `ClientboundStopSoundPacket` |
| `ClientboundSystemChatPacket` |
| `ClientboundTabListPacket` |
| `ClientboundTagQueryPacket` |
| `ClientboundTakeItemEntityPacket` |
| `ClientboundTeleportEntityPacket` |
| `ClientboundTestInstanceBlockStatus` |
| `ClientboundTickingStatePacket` |
| `ClientboundTickingStepPacket` |
| `ClientboundTrackedWaypointPacket` |
| `ClientboundUpdateAdvancementsPacket` |
| `ClientboundUpdateAttributesPacket` |
| `ClientboundUpdateMobEffectPacket` |
| `ClientboundUpdateRecipesPacket` |

### Serverbound (58 packets)

| Packet Class |
|---|
| `ServerboundAcceptTeleportationPacket` |
| `ServerboundAttackPacket` |
| `ServerboundBlockEntityTagQueryPacket` |
| `ServerboundChangeDifficultyPacket` |
| `ServerboundChangeGameModePacket` |
| `ServerboundChatAckPacket` |
| `ServerboundChatCommandPacket` |
| `ServerboundChatCommandSignedPacket` |
| `ServerboundChatPacket` |
| `ServerboundChatSessionUpdatePacket` |
| `ServerboundChunkBatchReceivedPacket` |
| `ServerboundClientCommandPacket` |
| `ServerboundClientTickEndPacket` |
| `ServerboundCommandSuggestionPacket` |
| `ServerboundConfigurationAcknowledgedPacket` |
| `ServerboundContainerButtonClickPacket` |
| `ServerboundContainerClickPacket` |
| `ServerboundContainerClosePacket` |
| `ServerboundContainerSlotStateChangedPacket` |
| `ServerboundDebugSubscriptionRequestPacket` |
| `ServerboundEditBookPacket` |
| `ServerboundEntityTagQueryPacket` |
| `ServerboundInteractPacket` |
| `ServerboundJigsawGeneratePacket` |
| `ServerboundLockDifficultyPacket` |
| `ServerboundMovePlayerPacket` |
| `ServerboundMoveVehiclePacket` |
| `ServerboundPaddleBoatPacket` |
| `ServerboundPickItemFromBlockPacket` |
| `ServerboundPickItemFromEntityPacket` |
| `ServerboundPlaceRecipePacket` |
| `ServerboundPlayerAbilitiesPacket` |
| `ServerboundPlayerActionPacket` |
| `ServerboundPlayerCommandPacket` |
| `ServerboundPlayerInputPacket` |
| `ServerboundPlayerLoadedPacket` |
| `ServerboundRecipeBookChangeSettingsPacket` |
| `ServerboundRecipeBookSeenRecipePacket` |
| `ServerboundRenameItemPacket` |
| `ServerboundSeenAdvancementsPacket` |
| `ServerboundSelectBundleItemPacket` |
| `ServerboundSelectTradePacket` |
| `ServerboundSetBeaconPacket` |
| `ServerboundSetCarriedItemPacket` |
| `ServerboundSetCommandBlockPacket` |
| `ServerboundSetCommandMinecartPacket` |
| `ServerboundSetCreativeModeSlotPacket` |
| `ServerboundSetGameRulePacket` |
| `ServerboundSetJigsawBlockPacket` |
| `ServerboundSetStructureBlockPacket` |
| `ServerboundSetTestBlockPacket` |
| `ServerboundSignUpdatePacket` |
| `ServerboundSpectateEntityPacket` |
| `ServerboundSwingPacket` |
| `ServerboundTeleportToEntityPacket` |
| `ServerboundTestInstanceBlockActionPacket` |
| `ServerboundUseItemOnPacket` |
| `ServerboundUseItemPacket` |
